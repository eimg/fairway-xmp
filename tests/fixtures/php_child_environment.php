<?php
// Exercise Laravel's reload-mode environment filtering with real Symfony Process
// (already shipped inside Composer), without downloading a framework in CI.
use Symfony\Component\Process\Process;

function configuration(): array
{
    $ini = php_ini_loaded_file();
    return [
        'ini' => $ini ? realpath($ini) : false,
        'memory' => ini_get('memory_limit'),
        'timezone' => ini_get('date.timezone'),
        'extensions' => get_loaded_extensions(),
        'phprc' => getenv('PHPRC'),
        'scan' => getenv('PHP_INI_SCAN_DIR'),
        'mibs' => getenv('MIBS'),
    ];
}

if (($argv[1] ?? '') === '--child') {
    echo json_encode(['configuration' => configuration(), 'project' => getenv('FAIRWAY_PROJECT_VALUE')], JSON_THROW_ON_ERROR);
    exit;
}

require 'phar://'.str_replace('\\', '/', $argv[1]).'/vendor/autoload.php';
$expected = configuration();
if (!$expected['ini']) {
    throw new RuntimeException('Parent did not load the managed PHP configuration.');
}

// Dotenv populates $_ENV with project values even with variables_order=GPCS.
// Laravel removes non-passthrough entries before starting PHP_BINARY directly.
// The child must inherit host settings and see freshly loaded project values.
foreach (['first', 'reloaded'] as $value) {
    $_ENV['FAIRWAY_PROJECT_VALUE'] = $value;
    $passthrough = ['APP_ENV', 'PATH', 'SYSTEMROOT'];
    $environment = [];
    foreach ($_ENV as $key => $entry) {
        $candidate = PHP_OS_FAMILY === 'Windows' ? strtoupper($key) : $key;
        $environment[$key] = in_array($candidate, $passthrough, true) ? $entry : false;
    }
    $process = new Process([PHP_BINARY, __FILE__, '--child'], null, $environment);
    $process->setTimeout(15);
    $process->mustRun();
    if ($process->getErrorOutput() !== '') {
        throw new RuntimeException('Child startup warnings: '.$process->getErrorOutput());
    }
    $result = json_decode($process->getOutput(), true, 512, JSON_THROW_ON_ERROR);
    if ($result['configuration'] !== $expected || $result['project'] !== false) {
        throw new RuntimeException('Child lost host configuration or retained a stale project value: '.$process->getOutput());
    }
    // Model Dotenv reading the current .env in the child, after stale values
    // were removed from its process environment.
    $process = new Process([PHP_BINARY, __FILE__, '--child'], null, array_merge($environment, ['FAIRWAY_PROJECT_VALUE' => $value]));
    $process->setTimeout(15);
    $process->mustRun();
    $result = json_decode($process->getOutput(), true, 512, JSON_THROW_ON_ERROR);
    if ($process->getErrorOutput() !== '' || $result['configuration'] !== $expected || $result['project'] !== $value) {
        throw new RuntimeException('Project environment update failed: '.$process->getOutput().$process->getErrorOutput());
    }
}
echo "PASS: child PHP retains managed configuration during project environment reloads\n";
