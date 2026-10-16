<?php
// Private stdin/stdout worker, executed only by Fairway's Rust backend.
// Database values stay strings so BIGINT/DECIMAL values survive JavaScript intact.
function ident($value) {
    if (!is_string($value) || $value === '' || strlen($value) > 64 || preg_match('/[\x00-\x1f\x7f]/', $value)) throw new Exception('Use a name of 1–64 characters without control characters.');
    return '`' . str_replace('`', '``', $value) . '`';
}
function user_db($name) {
    ident($name);
    if (in_array(strtolower($name), ['mysql','information_schema','performance_schema','sys'], true)) throw new Exception('System databases are not managed by the data browser.');
    return $name;
}
function rows($db, $sql, $args = []) { $s = $db->prepare($sql); $s->execute($args); return $s->fetchAll(PDO::FETCH_ASSOC); }
function table_columns($db, $schema, $table) {
    $result = rows($db, 'SELECT COLUMN_NAME AS name, COLUMN_TYPE AS type, IS_NULLABLE AS nullable, COLUMN_DEFAULT AS defaultValue, EXTRA AS extra, COLUMN_KEY AS columnKey, GENERATION_EXPRESSION AS expression, COLLATION_NAME AS collation, COLUMN_COMMENT AS comment FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY ORDINAL_POSITION', [$schema,$table]);
    if (!$result) throw new Exception('This table no longer exists. Refresh the database.');
    foreach ($result as &$c) { $c['nullable'] = $c['nullable'] === 'YES'; $c['binary'] = (bool)preg_match('/^(binary|varbinary|.*blob|bit|geometry|point|linestring|polygon|multipoint|multilinestring|multipolygon|geometrycollection)\b/i', $c['type']); $c['generated'] = $c['expression'] !== ''; }
    return $result;
}
function table_kind($db, $schema, $table) {
    $r = rows($db, 'SELECT TABLE_TYPE AS kind FROM information_schema.TABLES WHERE TABLE_SCHEMA=? AND TABLE_NAME=?', [$schema,$table]);
    if (!$r) throw new Exception('This table no longer exists.');
    return $r[0]['kind'];
}
function stable_key($db, $schema, $table, $columns) {
    if (table_kind($db,$schema,$table) !== 'BASE TABLE') return [];
    $indexes = rows($db, 'SELECT INDEX_NAME AS name, COLUMN_NAME AS col, SUB_PART AS prefix FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? AND NON_UNIQUE=0 ORDER BY (INDEX_NAME=\'PRIMARY\') DESC, INDEX_NAME, SEQ_IN_INDEX', [$schema,$table]);
    $groups = []; foreach ($indexes as $index) $groups[$index['name']][] = $index;
    $map = array_column($columns, null, 'name');
    foreach ($groups as $parts) {
        $key = []; foreach ($parts as $part) {
            $column = $map[$part['col']] ?? null;
            if (!$column || $column['nullable'] || $column['binary'] || $part['prefix'] !== null) { $key = []; break; }
            $key[] = $part['col'];
        }
        if ($key) return $key;
    }
    return [];
}
function column_sql($db, $c) {
    $type = strtoupper($c['type'] ?? '');
    $types = ['INT','BIGINT','SMALLINT','TINYINT','DECIMAL','FLOAT','DOUBLE','CHAR','VARCHAR','TEXT','LONGTEXT','BOOLEAN','DATE','DATETIME','TIMESTAMP','JSON','BLOB'];
    if (!in_array($type,$types,true)) throw new Exception('Choose a supported column type.');
    $sql = ident($c['name'] ?? '') . ' ' . $type;
    if (in_array($type,['CHAR','VARCHAR'],true)) {
        $length = filter_var($c['length'] ?? null,FILTER_VALIDATE_INT);
        if (!$length || $length < 1 || $length > ($type === 'CHAR' ? 255 : 16383)) throw new Exception('Enter a valid character length.');
        $sql .= "($length)";
    }
    if ($type === 'DECIMAL') {
        $p = filter_var($c['length'] ?? 10,FILTER_VALIDATE_INT); $s = filter_var($c['scale'] ?? 2,FILTER_VALIDATE_INT);
        if ($p === false || $s === false || $p < 1 || $p > 65 || $s < 0 || $s > min(30,$p)) throw new Exception('Choose DECIMAL precision 1–65 and a valid scale.');
        $sql .= "($p,$s)";
    }
    if (!empty($c['unsigned']) && in_array($type,['INT','BIGINT','SMALLINT','TINYINT','DECIMAL','FLOAT','DOUBLE'],true)) $sql .= ' UNSIGNED';
    $nullable = !empty($c['nullable']) && empty($c['primary']) && empty($c['autoIncrement']);
    $sql .= $nullable ? ' NULL' : ' NOT NULL';
    $mode = $c['defaultMode'] ?? 'none';
    if ($mode === 'null') { if (!$nullable) throw new Exception('A NOT NULL column cannot default to NULL.'); $sql .= ' DEFAULT NULL'; }
    elseif ($mode === 'value') { $literal=$db->quote((string)($c['defaultValue'] ?? '')); $sql.=' DEFAULT ' . (in_array($type,['TEXT','LONGTEXT','JSON','BLOB'],true) ? "($literal)" : $literal); }
    elseif ($mode === 'current_timestamp') {
        if (!in_array($type,['TIMESTAMP','DATETIME'],true)) throw new Exception('CURRENT_TIMESTAMP is available for timestamp and datetime columns.');
        $sql .= ' DEFAULT CURRENT_TIMESTAMP';
    } elseif ($mode !== 'none') throw new Exception('Unsupported default value.');
    if (!empty($c['autoIncrement'])) {
        if (!in_array($type,['INT','BIGINT','SMALLINT','TINYINT'],true) || empty($c['primary']) || $mode !== 'none') throw new Exception('Auto-increment needs an integer primary key without a default.');
        $sql .= ' AUTO_INCREMENT';
    }
    return $sql;
}
function result_set($s, $limit = 200) {
    $columns = []; for ($i=0;$i<$s->columnCount();$i++) $columns[] = $s->getColumnMeta($i)['name'];
    $data = []; $truncated = false;
    if ($columns) while (($row = $s->fetch(PDO::FETCH_NUM)) !== false) {
        if (count($data) >= $limit) { $truncated = true; break; }
        foreach ($row as &$v) if ($v !== null && !preg_match('//u',(string)$v)) $v = '0x' . bin2hex($v);
        unset($v); $data[] = $row;
    }
    return ['columns'=>$columns,'rows'=>$data,'affected'=>$s->rowCount(),'truncated'=>$truncated];
}
function run_request($db, $r) {
    $op = $r['op'] ?? '';
    if ($op === 'databases') return ['databases'=>array_column(rows($db,"SELECT SCHEMA_NAME AS name FROM information_schema.SCHEMATA WHERE SCHEMA_NAME NOT IN ('mysql','sys','information_schema','performance_schema') ORDER BY SCHEMA_NAME"),'name')];
    $schema = user_db($r['database'] ?? ''); $qdb = ident($schema);
    if ($op === 'create_database') { $db->exec("CREATE DATABASE $qdb CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"); return ['message'=>'Database created.']; }
    if ($op === 'drop_database') { $db->exec("DROP DATABASE $qdb"); return ['message'=>'Database deleted. A SQL backup was saved first.']; }
    if ($op === 'rename_database') {
        // MySQL has no atomic schema rename. Do not guess at migrating objects or application references.
        $name = user_db($r['name'] ?? '');
        $original = rows($db,'SELECT DEFAULT_CHARACTER_SET_NAME AS charset, DEFAULT_COLLATION_NAME AS collation FROM information_schema.SCHEMATA WHERE SCHEMA_NAME=?',[$schema]);
        if (!$original) throw new Exception('The database no longer exists. Refresh the database list.');
        $objects = rows($db,'SELECT (SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_SCHEMA=?) + (SELECT COUNT(*) FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA=?) + (SELECT COUNT(*) FROM information_schema.EVENTS WHERE EVENT_SCHEMA=?) AS n',[$schema,$schema,$schema]);
        if ((int)$objects[0]['n'] !== 0) throw new Exception('Only empty databases can be renamed in this first version. For populated databases, export SQL and import it into a new database; the original is preserved.');
        $charset=$original[0]['charset']; $collation=$original[0]['collation'];
        if (!preg_match('/^[a-zA-Z0-9_]+$/',$charset . $collation)) throw new Exception('Use SQL to preserve this database character set.');
        $db->exec('CREATE DATABASE ' . ident($name) . " CHARACTER SET $charset COLLATE $collation");
        // Preserve the original schema if anything appeared concurrently rather than drop new work.
        $check = rows($db,'SELECT (SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_SCHEMA=?) + (SELECT COUNT(*) FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA=?) + (SELECT COUNT(*) FROM information_schema.EVENTS WHERE EVENT_SCHEMA=?) AS n',[$schema,$schema,$schema]);
        if ((int)$check[0]['n'] !== 0) throw new Exception('The database changed during rename. Both databases were preserved.');
        $db->exec("DROP DATABASE $qdb"); return ['message'=>'Database renamed. Update project connection settings.'];
    }
    $db->exec("USE $qdb");
    if ($op === 'tables') return ['tables'=>rows($db,'SELECT TABLE_NAME AS name, TABLE_TYPE AS kind FROM information_schema.TABLES WHERE TABLE_SCHEMA=? ORDER BY TABLE_NAME',[$schema])];
    if ($op === 'query') {
        $sql = trim($r['sql'] ?? ''); if ($sql === '' || strlen($sql)>1024*1024) throw new Exception('Enter SQL up to 1 MiB.');
        $db->exec('SET SESSION sql_select_limit=201');
        // Native prepared statements cannot contain a multi-statement SQL script.
        // Form actions still use native bound parameters on their own connections.
        $db->setAttribute(PDO::ATTR_EMULATE_PREPARES,true);
        $db->setAttribute(PDO::MYSQL_ATTR_USE_BUFFERED_QUERY,false);
        try {
            $s = $db->query($sql); $sets=[];
            do { if (count($sets)>=10) throw new Exception('Run at most ten statements at a time.'); $sets[]=result_set($s); } while ($s->nextRowset());
        } catch (Throwable $e) { throw new Exception($e->getMessage() . ' Earlier statements may have completed. Refresh to check the current data.'); }
        return ['sets'=>$sets,'message'=>'Query completed.'];
    }
    $table = $r['table'] ?? ''; $qt = ident($table);
    if ($op === 'create_table') {
        $defs=[]; $primary=[]; $names=[]; $auto=0;
        $columns=$r['columns'] ?? []; if (!is_array($columns) || count($columns)<1 || count($columns)>100) throw new Exception('Create a table with 1–100 columns.');
        foreach ($columns as $c) { if (in_array(strtolower($c['name'] ?? ''),$names,true)) throw new Exception('Column names must be unique.'); $names[]=strtolower($c['name']); $defs[]=column_sql($db,$c); if (!empty($c['primary'])) $primary[]=ident($c['name']); if (!empty($c['autoIncrement'])) $auto++; }
        if ($auto>1) throw new Exception('Only one column can auto-increment.');
        if ($primary) $defs[]='PRIMARY KEY (' . implode(',',$primary) . ')';
        $db->exec("CREATE TABLE $qt (" . implode(',',$defs) . ')'); return ['message'=>'Table created.'];
    }
    $columns=table_columns($db,$schema,$table); $map=array_column($columns,null,'name');
    $key=stable_key($db,$schema,$table,$columns); $kind=table_kind($db,$schema,$table);
    if ($op === 'structure') { $ddl=rows($db,"SHOW CREATE TABLE $qt"); return ['columns'=>$columns,'key'=>$key,'kind'=>$kind,'sql'=>array_values($ddl[0])[1]]; }
    if ($op === 'rows') {
        $where=''; $args=[];
        if (($r['filterColumn'] ?? '') !== '' && ($r['filterValue'] ?? '') !== '') {
            if (!isset($map[$r['filterColumn']])) throw new Exception('Choose a valid filter column.');
            $where=' WHERE CAST(' . ident($r['filterColumn']) . ' AS CHAR) LIKE ?'; $args[]='%' . strtr($r['filterValue'],['!'=>'!!','%'=>'!%','_'=>'!_']) . '%'; $where .= " ESCAPE '!'";
        }
        $sort=$r['sort'] ?? ($key[0] ?? $columns[0]['name']); if (!isset($map[$sort])) throw new Exception('Choose a valid sort column.');
        $direction=($r['direction'] ?? 'ASC') === 'DESC' ? 'DESC' : 'ASC';
        $offset=max(0,min(10000000,(int)($r['offset'] ?? 0))); $limit=max(1,min(100,(int)($r['limit'] ?? 50)));
        $total=rows($db,"SELECT COUNT(*) AS n FROM $qt$where",$args)[0]['n'];
        $select=[]; foreach ($columns as $c) $select[]=($c['binary'] ? 'CONCAT(\'0x\',HEX(' . ident($c['name']) . '))' : ident($c['name'])) . ' AS ' . ident($c['name']);
        $order=ident($sort) . " $direction"; foreach ($key as $part) if ($part!==$sort) $order.=',' . ident($part) . " $direction";
        $s=$db->prepare('SELECT ' . implode(',',$select) . " FROM $qt$where ORDER BY $order LIMIT $limit OFFSET $offset"); $s->execute($args);
        return ['columns'=>$columns,'key'=>$key,'kind'=>$kind,'rows'=>$s->fetchAll(PDO::FETCH_ASSOC),'total'=>(string)$total,'offset'=>$offset,'limit'=>$limit];
    }
    if ($kind !== 'BASE TABLE') throw new Exception('Views are read-only in the data browser. Use the query editor for advanced changes.');
    if ($op === 'rename_table') { $db->exec("RENAME TABLE $qt TO " . ident($r['name'] ?? '')); return ['message'=>'Table renamed.']; }
    if ($op === 'drop_table') { $db->exec("DROP TABLE $qt"); return ['message'=>'Table deleted. A database backup was saved first.']; }
    if ($op === 'add_column' || $op === 'change_column') {
        $c=$r['column'] ?? []; $definition=column_sql($db,$c);
        if ($op === 'change_column') {
            $old=$r['oldName'] ?? ''; if (!isset($map[$old])) throw new Exception('The original column no longer exists.');
            if ($map[$old]['generated']) throw new Exception('Use the query editor to change generated columns.');
            if (preg_match('/on update|invisible|zerofill/i',$map[$old]['type'] . ' ' . $map[$old]['extra']) || (stripos($map[$old]['extra'],'DEFAULT_GENERATED')!==false && !preg_match('/^current_timestamp(?:\(\d*\))?$/i',$map[$old]['defaultValue'] ?? ''))) throw new Exception('Use SQL to preserve this advanced column definition.');
            // CHANGE COLUMN would otherwise silently remove comments and custom text collation.
            if ($map[$old]['collation'] && in_array(strtoupper($c['type']),['CHAR','VARCHAR','TEXT','LONGTEXT'],true)) {
                $collation=$map[$old]['collation']; if (!preg_match('/^[a-zA-Z0-9_]+$/',$collation)) throw new Exception('Unsupported collation. Use SQL to edit this column.');
                $definition.=' COLLATE ' . $collation;
            }
            $definition.=' COMMENT ' . $db->quote($map[$old]['comment']);
            $db->exec("ALTER TABLE $qt CHANGE COLUMN " . ident($old) . " $definition");
        } else { $db->exec("ALTER TABLE $qt ADD COLUMN $definition" . (!empty($c['primary']) ? ', ADD PRIMARY KEY (' . ident($c['name']) . ')' : '')); }
        return ['message'=>'Column saved.'];
    }
    if ($op === 'drop_column') { $db->exec("ALTER TABLE $qt DROP COLUMN " . ident($r['name'] ?? '')); return ['message'=>'Column deleted. A database backup was saved first.']; }
    if (!in_array($op,['insert','update','delete'],true)) throw new Exception('Unknown database operation.');
    $values=$r['values'] ?? []; if (!is_array($values)) throw new Exception('Invalid row values.');
    foreach ($values as $name=>$value) { if (!isset($map[$name]) || $map[$name]['generated'] || $map[$name]['binary']) throw new Exception('This column is read-only.'); if ($value !== null && !is_string($value)) throw new Exception('Row values must be text or SQL NULL.'); }
    if ($op === 'insert') {
        $sql="INSERT INTO $qt (" . implode(',',array_map('ident',array_keys($values))) . ') VALUES (' . implode(',',array_fill(0,count($values),'?')) . ')';
        $s=$db->prepare($sql); $s->execute(array_values($values)); return ['message'=>'Row inserted.'];
    }
    if (!$key) throw new Exception('This table has no non-null primary or unique key. Browse and insert are available; use SQL for ambiguous edits.');
    $original=$r['original'] ?? []; $where=[]; $args=[];
    foreach ($columns as $c) {
        $name=$c['name']; if (!array_key_exists($name,$original)) throw new Exception('Refresh this row before changing it.');
        $value=$original[$name]; if ($value!==null && !is_string($value)) throw new Exception('Invalid original row.');
        if ($c['binary'] && $value!==null) { if (!preg_match('/^0x([0-9a-f]{2})*$/i',$value)) throw new Exception('Invalid binary value.'); $value=hex2bin(substr($value,2)); }
        $where[]='CAST(' . ident($name) . ' AS BINARY) <=> CAST(? AS BINARY)'; $args[]=$value;
    }
    $db->beginTransaction();
    try {
        $found=rows($db,"SELECT 1 AS found FROM $qt WHERE " . implode(' AND ',$where) . ' LIMIT 2 FOR UPDATE',$args);
        if (count($found)!==1) throw new Exception('This row changed or was deleted. Refresh before saving.');
        if ($op==='delete') { $s=$db->prepare("DELETE FROM $qt WHERE " . implode(' AND ',$where) . ' LIMIT 1'); $s->execute($args); }
        elseif ($values) { $s=$db->prepare("UPDATE $qt SET " . implode(',',array_map(fn($name)=>ident($name).'=?',array_keys($values))) . ' WHERE ' . implode(' AND ',$where) . ' LIMIT 1'); $s->execute([...array_values($values),...$args]); }
        if (($op==='delete' || $values) && $s->rowCount()!==1) throw new Exception('This row changed or was deleted. Refresh before saving.');
        $db->commit();
    } catch (Throwable $e) { if ($db->inTransaction()) $db->rollBack(); throw $e; }
    return ['message'=>$op==='delete'?'Row deleted.':'Row saved.'];
}
try {
    if (!extension_loaded('pdo_mysql')) throw new Exception('The bundled PHP MySQL driver is unavailable. Check the PHP log and the selected runtime before retrying.');
    $input=json_decode(stream_get_contents(STDIN),true,512,JSON_THROW_ON_ERROR);
    $db=new PDO('mysql:host=127.0.0.1;port=' . (int)$input['port'] . ';charset=utf8mb4','root',$input['password'],[PDO::ATTR_ERRMODE=>PDO::ERRMODE_EXCEPTION,PDO::ATTR_STRINGIFY_FETCHES=>true,PDO::ATTR_EMULATE_PREPARES=>false,PDO::MYSQL_ATTR_MULTI_STATEMENTS=>true,PDO::MYSQL_ATTR_FOUND_ROWS=>true,PDO::ATTR_TIMEOUT=>3]);
    fwrite(STDERR,'FAIRWAY_CONNECTION_ID=' . $db->query('SELECT CONNECTION_ID()')->fetchColumn() . "\n");
    $db->exec('SET SESSION innodb_lock_wait_timeout=5'); $db->exec('SET SESSION lock_wait_timeout=5'); $db->exec('SET SESSION max_execution_time=15000');
    $start=microtime(true); $result=run_request($db,$input['request']); $result['elapsedMs']=(int)((microtime(true)-$start)*1000);
    $json=json_encode(['result'=>$result],JSON_THROW_ON_ERROR);
    if (strlen($json)>8*1024*1024) throw new Exception('The result is too large. Select fewer rows or columns.');
    echo $json;
} catch (Throwable $e) { echo json_encode(['error'=>$e->getMessage()],JSON_INVALID_UTF8_SUBSTITUTE); exit(1); }
