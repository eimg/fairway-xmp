// Fairway XMP procedures, paired English/Myanmar. UI labels match the application.
// Introductions are optional and reserved for prerequisites or consequential constraints.
export const groups = [
  {
    "id": "projects",
    "name": [
      "Serve project folders",
      "ပရောဂျက်စတင်ရန်"
    ]
  },
  {
    "id": "data",
    "name": [
      "Work with data & email",
      "Database နှင့် Mail ပိုင်း"
    ]
  },
  {
    "id": "tools",
    "name": [
      "Configure bundled tools",
      "အသင့်ပါဝင်သော Tool များ ပြင်ဆင်ရန်"
    ]
  },
  {
    "id": "fix",
    "name": [
      "Fix a problem",
      "ပြဿနာအဖြေရှာနည်း"
    ]
  },
  {
    "id": "management",
    "name": [
      "Manage Fairway XMP",
      "Fairway XMP စီမံရန်"
    ]
  },
  {
    "id": "reference",
    "name": [
      "Connection reference",
      "Connection အချက်အလက်များ"
    ]
  }
];

export const guides = [
  {
    "id": "nginx",
    "group": "projects",
    "title": [
      "Open a project from the document root",
      "Document Root မှ စတင်ခြင်း"
    ],
    "steps": [
      [
        "In <strong>Environment</strong>, choose <strong>Open folder</strong> to open <code>fairway/www</code> in the home directory. Place the existing project folder inside it.",
        "Start <strong>Nginx</strong> in Environment. Its PHP worker starts with it.",
        "Open <code>http://127.0.0.1:8800/project-name/</code>, replacing <code>project-name</code> with the folder name. Use the saved website port if it differs."
      ],
      [
        "<strong>Environment</strong> ရှိ <strong>Open folder</strong> ကို နှိပ်၍ Home directory အောက်ရှိ <code>fairway/www</code> ကို ဖွင့်ပါ။ အသုံးပြုမည့် ပရောဂျက် Folder ကို ထိုနေရာတွင် ထည့်ပါ။",
        "Environment တွင် <strong>Nginx</strong> ကို စတင်ပါ (PHP worker ကိုလည်း တစ်ပါတည်း အလိုအလျှောက် စတင်ပေးသည်)။",
        "<code>project-name</code> နေရာတွင် မိမိနှစ်သက်ရာ Folder အမည်ကို ထည့်၍ <code>http://127.0.0.1:8800/project-name/</code> ကို ဖွင့်ပါ။ Port ပြောင်းထားလျှင် ပြောင်းထားသော Port ကို သုံးပါ။"
      ]
    ],
    "check": [
      "The project folder is served at the localhost URL. MySQL and Mail can remain stopped unless needed separately.",
      "ပရောဂျက်ကို localhost လိပ်စာမှ ဖွင့်နိုင်မည်။ MySQL နှင့် Mail တိုကို မလိုလျှင် ရပ်ထားနိုင်သည်။"
    ],
    "image": "environment.png",
    "linux": [
      "Place the project folder in <code>~/fairway/www</code>. Run <code>fairway start nginx</code> and check <code>fairway status</code> for the website address.",
      "ပရောဂျက် Folder ကို <code>~/fairway/www</code> တွင် ထည့်ပါ။ <code>fairway start nginx</code> ကို Run ၍ Website လိပ်စာကို <code>fairway status</code> မှ ကြည့်ပါ။"
    ],
    "related": [
      "sites",
      "site-not-opening"
    ],
    "trouble": [
      [
        [
          "The project has private files or a separate public folder",
          "ပရောဂျက်တွင် ထပ်ဆင့် Public Folder ပါရှိလျှင်"
        ],
        [
          "Folders inside <code>fairway/www</code> are served through localhost. Keep projects with private files outside that directory and <a href=\"#sites\">register a site</a> with only the public folder as its web root.",
          "<code>fairway/www</code> အောက်ရှိ Folder များကို localhost မှ တိုက်ရိုက်ဖွင့်နိုင်သည်။ ထပ်ဆင့်ဖိုဒါပါသော ပရောဂျက်များကို တိုက်ရိုက်ဖွင့်လို့ပါက <a href=\"#sites\">Site သတ်မှတ်ပါ</a>။"
        ]
      ]
    ]
  },
  {
    "id": "sites",
    "group": "projects",
    "title": [
      "Assign a .test address to a project",
      "ပရောဂျက်အတွက် .test လိပ်စာ သတ်မှတ်ရန်"
    ],
    "steps": [
      [
        "Open <strong>Manage Sites → Add site</strong>.",
        "Enter the site name and a domain such as <code>shop.test</code>.",
        "Choose the folder to serve as <strong>Web root</strong> and select <strong>App type</strong>. For Laravel, select its <code>public</code> folder and <strong>Laravel / front controller</strong>. Keep the parent project outside <code>fairway/www</code> so private files are not also served through localhost.",
        "Save and complete address setup. On macOS, allow Fairway XMP in <strong>System Settings → General → Login Items & Extensions</strong>. On Windows, approve the administrator prompt.",
        "Choose <strong>Start web server</strong> if needed, then the saved site’s <strong>Open</strong> action. The displayed address includes the website port, normally <code>http://shop.test:8800/</code>."
      ],
      [
        "<strong>Manage Sites → Add site</strong> ကို ဖွင့်ပါ။",
        "Site အမည်နှင့် <code>shop.test</code> ကဲ့သို့ Domain ကို ထည့်ပါ။",
        "နှစ်သက်ရာ Folder ကို <strong>Web root</strong> အဖြစ် ရွေး၍ <strong>App type</strong> ကို သတ်မှတ်ပါ။ Laravel အတွက် <code>public</code> Folder ကိုရွေးပြီး <strong>Laravel / front controller</strong> ကို ရွေးပါ။ localhost မှ ဖွင့်မရစေလိုပါက ပရောဂျက် Folder ကို <code>fairway/www</code> ၏ ပြင်ပတွင် ထားပါ။",
        "သိမ်းဆည်းပြီး လိပ်စာသတ်မှတ်မှုအတွက် လိုအပ်သည့် ခွင့်ပြုချက်ကို ပေးပါ။ macOS တွင် <strong>System Settings → General → Login Items & Extensions</strong> မှ Fairway XMP ကို ခွင့်ပြုပါ။ Windows တွင် Administrator ခွင့်ပြုချက် တောင်းဆိုမှုကို အတည်ပြုပါ။",
        "လိုအပ်လျှင် <strong>Start web server</strong> ကို နှိပ်၍ Site နှင့်အတူ ပူးတွဲပေးထားသော <strong>Open</strong> ကို နှိပ်ပါ။ ပြထားသော လိပ်စာတွင် Website Port ပါဝင်သည်။ မူလလိပ်စာမှာ <code>http://shop.test:8800/</code> ဖြစ်သည်။"
      ]
    ],
    "check": [
      "The saved .test address serves the selected web root. Adding a site keeps the project files in their existing location.",
      "သတ်မှတ်ထားသော .test လိပ်စာမှ ရွေးထားသော ဖိုဒါကို ဖွင့်နိုင်မည်။ ဤကဲ့သို့ Site ထည့်သွင်းခြင်းကြောင့် ပရောဂျက်ဖိုင်များ နေရာပြောင်းသွားမည် မဟုတ်ပါ။"
    ],
    "image": "sites.png",
    "trouble": [
      [
        [
          "Waiting for macOS approval",
          "macOS ခွင့်ပြုချက်ကို စောင့်ဆိုင်းနေလျှင်"
        ],
        [
          "Allow Fairway XMP in <strong>System Settings → General → Login Items & Extensions</strong>, then choose <strong>Check approval</strong>. <strong>Open System Settings</strong> returns to the approval page. The saved site registration is retained while approval is pending.",
          "<strong>System Settings → General → Login Items & Extensions</strong> တွင် Fairway XMP ကို ခွင့်ပြုပြီး <strong>Check approval</strong> ကို နှိပ်ပါ။ ခွင့်ပြုချက်ပေးရမည့် စာမျက်နှာသို့ ပြန်သွားရန် <strong>Open System Settings</strong> ကို နှိပ်ပါ။ ခွင့်ပြုချက် စောင့်ဆိုင်းနေစဉ် Site ကို သိမ်းထားပြီး ဖြစ်သည်။"
        ]
      ],
      [
        [
          "Address setup was cancelled or failed",
          "လိပ်စာသတ်မှတ်မှုကို ပယ်ဖျက်မိလျှင် သို့မဟုတ် မအောင်မြင်လျှင်"
        ],
        [
          "Choose <strong>Set up addresses</strong> and complete the permission prompt. For other failures, follow <a href=\"#site-not-opening\">Fix a site that won’t open</a>.",
          "<strong>Set up addresses</strong> ကို နှိပ်၍ လိုအပ်သည့် ခွင့်ပြုချက်ကို ပေးပါ။ အခြားပြဿနာများအတွက် <a href=\"#site-not-opening\">Site ဖွင့်မရသည့်အခါ ဖြေရှင်းနည်း</a> ကို ကြည့်ပါ။"
        ]
      ]
    ],
    "linux": [
      "Register your project with <code>fairway site add shop ~/code/shop/public --name \"My Shop\" --index</code>. Start services with <code>fairway start</code>; <code>fairway status</code> lists the URL. Address changes ask for your sudo password. Retry with <code>fairway site hosts</code> if needed.",
      "<code>fairway site add shop ~/code/shop/public --name \"My Shop\" --index</code> ဖြင့် Site သတ်မှတ်ပါ။ Service များ စတင်ရန် <code>fairway start</code> ကို Run ၍ လိပ်စာကို <code>fairway status</code> မှ ကြည့်ပါ။ လိပ်စာပြောင်းရန် Sudo password တောင်းမည်။ လိုအပ်လျှင် <code>fairway site hosts</code> ဖြင့် ပြန်လည်လုပ်ဆောင်ပါ။"
    ],
    "related": [
      "https",
      "terminal",
      "mysql",
      "site-not-opening"
    ]
  },
  {
    "id": "https",
    "group": "projects",
    "title": [
      "Use HTTPS for a local site",
      ".test site အတွက် HTTPS သုံးရန်"
    ],
    "steps": [
      [
        "In <strong>Manage Sites</strong>, turn on the site’s <strong>HTTPS</strong> switch.",
        "On first use, approve trust for Fairway XMP’s local certificate authority. macOS uses the login keychain; Windows uses the user certificate store.",
        "Open the site’s displayed HTTPS address, normally <code>https://shop.test:8443/</code>."
      ],
      [
        "<strong>Manage Sites</strong> တွင် သက်ဆိုင်ရာ Site ၏ <strong>HTTPS</strong> ကို ဖွင့်ပါ။",
        "ပထမဆုံး အသုံးပြုစဉ် Fairway XMP ၏ Local certificate authority ကို ယုံကြည်ရန် ခွင့်ပြုချက်ပေးပါ။ macOS တွင် Login keychain၊ Windows တွင် User certificate store ကို အသုံးပြုသည်။",
        "Site နှင့်အတူ ပူးတွဲပြထားသော HTTPS လိပ်စာကို ဖွင့်ပါ။ မူလလိပ်စာ ဥပမာ မှာ <code>https://shop.test:8443/</code> ဖြစ်သည်။"
      ]
    ],
    "check": [
      "The HTTPS page opens without a certificate warning. The HTTP address still works.",
      "Browser ကပေးသော Certificate လုံခြုံရေး သတိပေးချက် မရှိတော့ဘဲ HTTPS နှင့် စမ်းသပ်နိုင်မည်။ HTTP လိပ်စာကိုလည်း ဆက်သုံးနိုင်သည်။"
    ],
    "trouble": [
      [
        [
          "The browser reports a certificate problem",
          "Browser တွင် Certificate ပြဿနာ ပြနေလျှင်"
        ],
        [
          "Check that you used the saved .test domain and HTTPS port. Complete any pending trust setup; use <strong>Update certificate trust</strong> if the app offers it, then reopen the browser. On Linux, check the browser’s own certificate settings if the warning remains.",
          "သိမ်းထားသော .test Domain နှင့် HTTPS Port ကို သုံးထားကြောင်း စစ်ပါ။ Certificate ယုံကြည်ရန် ခွင့်ပြုချက်ပေးမှု မပြီးသေးလျှင် အပြီးသတ်ပါ။ App တွင် <strong>Update certificate trust</strong> ပြထားလျှင် နှိပ်၍ Browser ကို ပြန်ဖွင့်ပါ။ Linux တွင် သတိပေးချက် ဆက်ရှိနေလျှင် Browser ၏ Certificate settings ကို စစ်ပါ။"
        ]
      ],
      [
        [
          "Remove local certificate trust",
          "Local certificate ယုံကြည်မှုကို ဖယ်ရှားရန်"
        ],
        [
          "On desktop, turn off HTTPS for the last HTTPS site or remove that site. Fairway XMP also removes the shared trust. If cleanup is cancelled, HTTPS stays off; choose <strong>Remove certificate trust</strong> to retry.",
          "Desktop App တွင် HTTPS သုံးထားသော နောက်ဆုံး Site ၏ HTTPS ကို ပိတ်ပါ သို့မဟုတ် ထို Site ကို ဖယ်ရှားပါ။ မျှဝေသုံးထားသော Certificate ကိုပါ Fairway XMP က ဖယ်ရှားပေးသည်။ ဖယ်ရှားမှုကို ပယ်ဖျက်မိလျှင် HTTPS ပိတ်ထားဆဲဖြစ်၍ <strong>Remove certificate trust</strong> ဖြင့် ပြန်လည်လုပ်ဆောင်နိုင်သည်။"
        ]
      ]
    ],
    "linux": [
      "Run <code>fairway site https shop on</code>. Certificate changes require your sudo password. Use <code>off</code> in place of <code>on</code> to disable HTTPS.",
      "<code>fairway site https shop on</code> ကို Run ပါ။ Certificate ပြောင်းလဲရန် Sudo password လိုအပ်သည်။ HTTPS ပိတ်ရန် <code>on</code> အစား <code>off</code> ကို သုံးပါ။"
    ],
    "intro": [
      "Requires a <a href=\"#sites\">saved .test site</a> with completed address setup.",
      "လိပ်စာသတ်မှတ်မှု ပြီးစီးထားသော <a href=\"#sites\">.test Site</a> ရှိရန် လိုအပ်သည်။"
    ]
  },
  {
    "id": "mysql",
    "group": "data",
    "title": [
      "Connect an application to Fairway XMP’s MySQL",
      "Fairway XMP ၏ MySQL သို့ ချိတ်ဆက်ရန်"
    ],
    "steps": [
      [
        "Start <strong>MySQL</strong> in <strong>Environment</strong> and wait until it is ready.",
        "In <strong>Database connection</strong>, use the copy button to copy the connection values. A fresh installation uses:<pre><code>DB_HOST=127.0.0.1\nDB_PORT=13306\nDB_DATABASE=fairway\nDB_USERNAME=fairway\nDB_PASSWORD=fairway</code></pre>",
        "Apply these values in the application’s database configuration. Use the displayed port and current credentials if they differ from the defaults."
      ],
      [
        "<strong>Environment</strong> တွင် <strong>MySQL</strong> ကို စတင်၍ အသင့်ဖြစ်သည်အထိ စောင့်ပါ။",
        "<strong>Database connection</strong> ရှိ Copy ခလုတ်ကို နှိပ်၍ Connection တန်ဖိုးများကို ကူးယူပါ။ မူလတန်ဖိုးများမှာ အောက်ပါအတိုင်း ဖြစ်သည်။<pre><code>DB_HOST=127.0.0.1\nDB_PORT=13306\nDB_DATABASE=fairway\nDB_USERNAME=fairway\nDB_PASSWORD=fairway</code></pre>",
        "ထိုတန်ဖိုးများကို App ၏ Database configuration တွင် သတ်မှတ်ပါ။ နမူနာပြထားသော မူလတန်ဖိုးများနှင့် မတူလျှင် လက်ရှိပြထားသော Port နှင့် Username/Password ကို သုံးပါ။"
      ]
    ],
    "check": [
      "The application connects to the MySQL service using the host, port and credentials shown in Fairway XMP.",
      "Fairway XMP တွင် ပြထားသော Host၊ Port နှင့် Username/Password ဖြင့် App မှ MySQL သို့ ချိတ်ဆက်နိုင်မည်။"
    ],
    "trouble": [
      [
        [
          "Connection refused or access denied",
          "Connection refused သို့မဟုတ် Access denied ပြနေလျှင်"
        ],
        [
          "For connection refused, check MySQL is running and compare the port with <strong>Environment → Database connection</strong>. For access denied, check your saved credentials. Use <code>127.0.0.1</code> as host to connect over TCP.",
          "Connection refused အတွက် MySQL စတင်ထားကြောင်း စစ်၍ Port ကို <strong>Environment → Database connection</strong> နှင့် တိုက်စစ်ပါ။ Access denied အတွက် သိမ်းထားသော Username/Password ကို စစ်ပါ။ TCP ဖြင့် ချိတ်ဆက်ရန် Host အဖြစ် <code>127.0.0.1</code> ကို သုံးပါ။"
        ]
      ],
      [
        [
          "Use a separate database",
          "သီးခြား Database သုံးရန်"
        ],
        [
          "Create the database in <strong>Browse Data</strong> or phpMyAdmin and set its name in the application’s connection settings. Fairway XMP does not rewrite application configuration when a database or port changes.",
          "<strong>Browse Data</strong> သို့မဟုတ် phpMyAdmin တွင် Database ဖန်တီး၍ ၎င်း၏အမည်ကို App ၏ Connection settings တွင် သတ်မှတ်ပါ။ Database သို့မဟုတ် Port ပြောင်းလဲခြင်းကြောင့် App ၏ Configuration ကို Fairway XMP က အလိုအလျောက် ပြင်ပေးမည် မဟုတ်ပါ။"
        ]
      ]
    ],
    "linux": [
      "Run <code>fairway start mysql</code>. The connection values are the same; <code>fairway ports</code> shows your current ports.",
      "<code>fairway start mysql</code> ကို Run ပါ။ Connection တန်ဖိုးများ အတူတူဖြစ်သည်။ လက်ရှိ Port များကို <code>fairway ports</code> ဖြင့် ကြည့်နိုင်သည်။"
    ],
    "related": [
      "phpmyadmin",
      "ports"
    ]
  },
  {
    "id": "phpmyadmin",
    "group": "data",
    "title": [
      "Browse and edit database records",
      "Database record များ ကြည့်ရန်နှင့် ပြင်ဆင်ရန်"
    ],
    "steps": [
      [
        "Start MySQL, open <strong>Browse Data</strong>, and choose your database and table.",
        "Open a row’s eye action to read its full values. Use <strong>Edit</strong> to change a record, or <strong>Insert row</strong> to add one.",
        "To inspect columns, indexes or relationships, open the table’s <strong>Structure</strong> tab. Use the SQL editor when you need to filter records."
      ],
      [
        "MySQL ကို စတင်၍ <strong>Browse Data</strong> ကို ဖွင့်ပါ။ အသုံးပြုမည့် Database နှင့် Table ကို ရွေးပါ။",
        "တန်ဖိုးအပြည့်အစုံ ကြည့်ရန် Row ၏ မျက်လုံးပုံ ခလုတ်ကို နှိပ်ပါ။ Record ပြင်ဆင်ရန် <strong>Edit</strong>၊ အသစ်ထည့်ရန် <strong>Insert row</strong> ကို သုံးပါ။",
        "Column၊ Index နှင့် Relationship များကို ကြည့်ရန် Table ၏ <strong>Structure</strong> Tab ကို ဖွင့်ပါ။ Record များ စစ်ထုတ်ရန် SQL editor ကို သုံးနိုင်သည်။"
      ]
    ],
    "check": [
      "Saved row changes appear in the selected table after refresh. These edits affect the original database.",
      "Refresh လုပ်ပြီးနောက် ပြင်ဆင်သိမ်းထားသော Row များကို ရွေးထားသော Table တွင် တွေ့ရမည်။ ပြင်ဆင်မှုများသည် မူလ Database တွင် တိုက်ရိုက် သက်ရောက်သည်။"
    ],
    "image": "data-browser.png",
    "trouble": [
      [
        [
          "A row cannot be edited",
          "Row ကို ပြင်ဆင်မရလျှင်"
        ],
        [
          "Views are read-only. Updating or deleting a row needs a usable primary key or non-null unique key; keyless tables still allow browsing and inserting. If a record changed elsewhere, refresh it before trying again.",
          "View များကို ကြည့်ရှုရန်သာ သုံးနိုင်သည်။ Row ပြင်ဆင်ခြင်း သို့မဟုတ် ဖျက်ခြင်းအတွက် အသုံးပြုနိုင်သော Primary key သို့မဟုတ် Non-null unique key လိုအပ်သည်။ Key မရှိသော Table များတွင် ကြည့်ရှုခြင်းနှင့် အသစ်ထည့်ခြင်း လုပ်နိုင်သည်။ Record ကို အခြားနေရာမှ ပြင်ဆင်ထားလျှင် Refresh လုပ်ပြီး ပြန်ကြိုးစားပါ။"
        ]
      ],
      [
        [
          "Open phpMyAdmin",
          "phpMyAdmin ဖွင့်ရန်"
        ],
        [
          "Start both Nginx/PHP and MySQL. Open <code>http://127.0.0.1:8801/</code> and sign in with <code>fairway</code> / <code>fairway</code> on a fresh installation. Root login is disabled.",
          "Nginx/PHP နှင့် MySQL နှစ်ခုလုံးကို စတင်ပါ။ <code>http://127.0.0.1:8801/</code> ကို ဖွင့်၍ မူလ Username/Password ဖြစ်သော <code>fairway</code> / <code>fairway</code> ဖြင့် ဝင်ပါ။ Root login ကို ခွင့်မပြုထားပါ။"
        ]
      ]
    ],
    "related": [
      "sqlite",
      "import-sql",
      "backups"
    ],
    "intro": [
      "Browse Data is available on Windows and macOS. phpMyAdmin is available on all platforms.",
      "Browse Data ကို Windows နှင့် macOS တွင် သုံးနိုင်သည်။ phpMyAdmin ကို Platform အားလုံးတွင် သုံးနိုင်သည်။"
    ]
  },
  {
    "id": "sqlite",
    "group": "data",
    "title": [
      "Open a project’s SQLite file",
      "SQLite ဖိုင် ဖွင့်ရန်"
    ],
    "steps": [
      [
        "Open <strong>Browse Data → Open SQLite file…</strong> and select the database file.",
        "Choose a table to browse its rows or structure.",
        "Use <strong>Back up SQLite file</strong> on the open-file card to save a snapshot before editing. <strong>Open backups</strong> opens the backup directory. Choose <strong>Close SQLite file</strong> to release the file when finished."
      ],
      [
        "<strong>Browse Data → Open SQLite file…</strong> မှ Database ဖိုင်ကို ရွေးပါ။",
        "Row သို့မဟုတ် Structure ကြည့်ရန် Table ကို ရွေးပါ။",
        "မပြင်ဆင်မီ ဖွင့်ထားသောဖိုင်၏ အချက်အလက်နေရာရှိ <strong>Back up SQLite file</strong> ကို နှိပ်၍ Backup သိမ်းပါ။ Backup Folder ကို <strong>Open backups</strong> မှ ဖွင့်နိုင်သည်။ အသုံးပြုပြီးလျှင် ဖိုင်ပိတ်ရန် <strong>Close SQLite file</strong> ကို နှိပ်ပါ။"
      ]
    ],
    "check": [
      "The selected SQLite file appears in the open-file card and its tables are listed in Browse Data.",
      "ဖွင့်ထားသော SQLite ဖိုင်၏ အချက်အလက်နှင့် Table စာရင်းကို Browse Data တွင် တွေ့ရမည်။"
    ],
    "trouble": [
      [
        [
          "Restore a SQLite backup",
          "SQLite backup မှ ပြန်ထည့်ရန်"
        ],
        [
          "Stop the project and other programs using the database, and close it in Browse Data. Keep a copy of the current file, then replace it with the saved snapshot using your file manager. Reopen it and check the records. MySQL environment backups do not include SQLite files.",
          "ပရောဂျက်နှင့် Database သုံးနေသော အခြား Program များကို ရပ်၍ Browse Data တွင် ဖိုင်ကို ပိတ်ပါ။ လက်ရှိဖိုင်ကို မိတ္တူသိမ်းထားပြီး File manager မှ Backup ဖိုင်ဖြင့် အစားထိုးပါ။ ပြန်ဖွင့်၍ Record များကို စစ်ပါ။ MySQL Backup တွင် SQLite ဖိုင်များ မပါဝင်ပါ။"
        ]
      ]
    ],
    "related": [
      "backups"
    ],
    "intro": [
      "Available on Windows and macOS. No running services are required; edits affect the selected file directly.",
      "Windows နှင့် macOS တွင် သုံးနိုင်သည်။ Service များ စတင်ထားရန် မလိုအပ်ပါ။ ပြင်ဆင်မှုများသည် ရွေးထားသောဖိုင်တွင် တိုက်ရိုက် သက်ရောက်သည်။"
    ]
  },
  {
    "id": "import-sql",
    "group": "data",
    "title": [
      "Import a MySQL SQL file",
      "SQL ဖိုင်ကို MySQL သို့ Import လုပ်ရန်"
    ],
    "steps": [
      [
        "Start MySQL. In <strong>Browse Data</strong>, choose the target database; create it first if needed.",
        "Choose <strong>Database actions → Import SQL file…</strong> and pick an uncompressed SQL file. Review the dump first: SQL can name or switch to databases other than the selected target.",
        "Type the target database name to confirm and start the import. Fairway XMP must finish a backup of that database before importing; keep the backup path shown in the result."
      ],
      [
        "MySQL ကို စတင်ပါ။ <strong>Browse Data</strong> တွင် ထည့်သွင်းမည့် Database ကို ရွေးပါ။ မရှိသေးလျှင် အရင်ဖန်တီးပါ။",
        "<strong>Database actions → Import SQL file…</strong> မှ ချုံ့မထားသော SQL ဖိုင်ကို ရွေးပါ။ SQL ထဲတွင် အခြား Database များ၏ အမည် သို့မဟုတ် Database ပြောင်းသုံးရန် ညွှန်ကြားချက် ပါနိုင်သဖြင့် ဖိုင်ကို အရင်စစ်ဆေးပါ။",
        "အတည်ပြုရန် Database အမည်ကို ရိုက်ထည့်၍ Import စတင်ပါ။ Fairway XMP က ထို Database ကို Backup အောင်မြင်စွာ သိမ်းပြီးမှ Import လုပ်သည်။ ရလဒ်တွင် ပြထားသော Backup ဖိုင်တည်နေရာကို မှတ်ထားပါ။"
      ]
    ],
    "check": [
      "Imported tables and records appear in the target database after refresh. The result includes the pre-import backup path.",
      "Refresh လုပ်ပြီးနောက် ထည့်သွင်းထားသော Table နှင့် Record များကို ရွေးထားသော Database တွင် တွေ့ရမည်။ ရလဒ်တွင် Import မလုပ်မီ သိမ်းထားသော Backup ဖိုင်တည်နေရာ ပါဝင်သည်။"
    ],
    "trouble": [
      [
        [
          "Failed or cancelled import",
          "Import မအောင်မြင်လျှင် သို့မဟုတ် ပယ်ဖျက်မိလျှင်"
        ],
        [
          "Completed statements stay committed. Refresh and inspect the database before retrying; cancellation does not undo changes. Only the selected target is backed up automatically. To recover a full environment dump, follow <a href=\"#restore\">Restore MySQL databases</a>.",
          "ပြီးစီးသွားသော SQL statement များ၏ ပြောင်းလဲမှုများ ဆက်ရှိနေမည်။ Cancel လုပ်ခြင်းက ထိုပြောင်းလဲမှုများကို နောက်ပြန်ဆုတ်ပေးမည် မဟုတ်ပါ။ ပြန်မကြိုးစားမီ Refresh လုပ်၍ Database ကို စစ်ပါ။ ရွေးထားသော Database တစ်ခုတည်းကိုသာ အလိုအလျောက် Backup သိမ်းသည်။ Database အားလုံးပါသော Backup ပြန်ထည့်ရန် <a href=\"#restore\">MySQL Database များ ပြန်ထည့်နည်း</a> ကို ကြည့်ပါ။"
        ]
      ]
    ],
    "linux": [
      "Start Nginx/PHP and MySQL, then import through phpMyAdmin at <code>http://127.0.0.1:8801/</code>. For a whole-environment backup, use the <a href=\"#restore\">restore guide</a>. The desktop <code>fairway db</code> commands are not part of the Linux edition.",
      "Nginx/PHP နှင့် MySQL ကို စတင်၍ <code>http://127.0.0.1:8801/</code> ရှိ phpMyAdmin မှ Import လုပ်ပါ။ Database အားလုံးပါသော Backup အတွက် <a href=\"#restore\">ပြန်ထည့်နည်း</a> ကို ကြည့်ပါ။ Desktop ၏ <code>fairway db</code> Command များသည် Linux edition တွင် မပါဝင်ပါ။"
    ],
    "related": [
      "backups",
      "restore"
    ],
    "intro": [
      "Import can replace existing tables. Stop other applications writing to the affected databases before importing.",
      "Import လုပ်ခြင်းက ရှိပြီးသား Table များကို အစားထိုးနိုင်သည်။ မလုပ်ဆောင်မီ သက်ဆိုင်ရာ Database များသို့ ရေးသားနေသော အခြား App များကို ရပ်ပါ။"
    ]
  },
  {
    "id": "backups",
    "group": "data",
    "title": [
      "Back up databases",
      "Database များ Backup သိမ်းရန်"
    ],
    "steps": [
      [
        "Start MySQL. In <strong>Environment → Database connection</strong>, choose <strong>Back up databases</strong>.",
        "Wait for the completed-backup message. Open <strong>Settings → Open backups</strong> to find the completed <code>.sql</code> file. A <code>.partial</code> file is unfinished.",
        "Copy the completed SQL file to a separate backup location. The default backups directory is stored alongside Fairway XMP’s database data. For SQLite, use <a href=\"#sqlite\">Back up SQLite file</a> instead."
      ],
      [
        "MySQL ကို စတင်ပါ။ <strong>Environment → Database connection</strong> ရှိ <strong>Back up databases</strong> ကို နှိပ်ပါ။",
        "Backup ပြီးစီးကြောင်း အကြောင်းကြားချက်ကို စောင့်ပါ။ ပြီးစီးသော <code>.sql</code> ဖိုင်ကို <strong>Settings → Open backups</strong> မှ ရှာပါ။ <code>.partial</code> ဖိုင်သည် မပြီးစီးသေးသော ဖိုင်ဖြစ်သည်။",
        "ပြီးစီးသော SQL ဖိုင်ကို သီးခြား Backup နေရာသို့ ကူးယူပါ။ မူလ Backup Folder သည် Fairway XMP ၏ Database data နှင့်အတူ သိမ်းထားခြင်း ဖြစ်သည်။ SQLite အတွက် <a href=\"#sqlite\">Back up SQLite file</a> ကို သုံးပါ။"
      ]
    ],
    "check": [
      "The completed SQL file is present in the backup directory and the operation reports success.",
      "Backup ပြီးစီးကြောင်း ပြသပြီး SQL ဖိုင်ကို Backup Folder တွင် တွေ့ရမည်။"
    ],
    "trouble": [
      [
        [
          "A Windows backup fails with a Unicode-name error",
          "Windows တွင် Unicode အမည်ကြောင့် Backup မအောင်မြင်လျှင်"
        ],
        [
          "On Windows systems with a non-UTF-8 process code page, whole-environment backups require ASCII database names. phpMyAdmin can make a manual SQL export for other names. Keep that export; it does not bypass the updater’s automatic-backup requirement.",
          "Process code page က UTF-8 မဟုတ်သော Windows စနစ်များတွင် Database အားလုံးကို Backup သိမ်းရန် Database အမည်များသည် ASCII ဖြစ်ရမည်။ အခြားအမည်များအတွက် phpMyAdmin မှ SQL Export ကိုယ်တိုင်လုပ်၍ သိမ်းထားနိုင်သည်။ သို့သော် Updater ၏ အလိုအလျောက် Backup လိုအပ်ချက်ကို ကျော်လွှားပေးမည် မဟုတ်ပါ။"
        ]
      ]
    ],
    "linux": [
      "Run <code>fairway backup</code> with MySQL running, then copy the completed SQL file to your backup location.",
      "MySQL စတင်ထားပြီး <code>fairway backup</code> ကို Run ပါ။ ပြီးစီးသော SQL ဖိုင်ကို သီးခြား Backup နေရာသို့ ကူးယူပါ။"
    ],
    "related": [
      "restore",
      "sqlite"
    ],
    "intro": [
      "Back up databases exports MySQL data, views, routines, events and triggers. It excludes MySQL accounts, project files and SQLite databases.",
      "Back up databases သည် MySQL Data၊ View၊ Routine၊ Event နှင့် Trigger များကို Export လုပ်ပေးသည်။ MySQL account၊ ပရောဂျက်ဖိုင်နှင့် SQLite Database များ မပါဝင်ပါ။"
    ]
  },
  {
    "id": "restore",
    "group": "data",
    "title": [
      "Restore MySQL databases from a backup",
      "Backup မှ MySQL Database များ ပြန်ထည့်ရန်"
    ],
    "steps": [
      [
        "Start MySQL and stop apps writing to the databases. <a href=\"#backups\">Back up their current contents</a> and copy that backup somewhere safe before restoring: matching tables can be replaced.",
        "Expand the command for your platform below. Replace the example backup filename and any custom installation path, then run it in your terminal. Keep the private <code>admin.cnf</code> file on your own device.",
        "Wait for the command to finish. If it reports an SQL error, investigate it before retrying; some statements may already have changed data. Do not add <code>--force</code>."
      ],
      [
        "MySQL ကို စတင်၍ Database သို့ ရေးသားနေသော App များကို ရပ်ပါ။ အမည်တူ Table များ အစားထိုးခံရနိုင်သဖြင့် လက်ရှိဒေတာကို <a href=\"#backups\">Backup သိမ်း</a>၍ လုံခြုံသော သီးခြားနေရာတွင် ကူးယူထားပါ။",
        "အောက်တွင် သက်ဆိုင်ရာ Platform ၏ Command ကို ဖွင့်ကြည့်ပါ။ နမူနာ Backup ဖိုင်အမည်နှင့် ကိုယ်ပိုင် Installation path ရှိလျှင် ပြင်ဆင်၍ Terminal တွင် Run ပါ။ သီးသန့်ဖိုင်ဖြစ်သော <code>admin.cnf</code> ကို မိမိ၏ကွန်ပျူတာတွင်သာ ထားပါ။",
        "Command ပြီးသည်အထိ စောင့်ပါ။ SQL Error ပြလျှင် Statement အချို့က ဒေတာကို ပြောင်းလဲပြီး ဖြစ်နိုင်သဖြင့် ပြန်မကြိုးစားမီ အကြောင်းရင်းကို စစ်ပါ။ <code>--force</code> မထည့်ပါနှင့်။"
      ]
    ],
    "platformCommands": [
      {
        "title": [
          "macOS · Terminal",
          "macOS · Terminal"
        ],
        "command": "\"/Applications/Fairway XMP.app/Contents/Resources/runtime/mysql/bin/mysql\" \\\n  --defaults-file=\"$HOME/Library/Application Support/dev.fairway.environment/config/admin.cnf\" \\\n  --default-character-set=utf8mb4 < \"/path/to/fairway-backup.sql\""
      },
      {
        "title": [
          "Windows · Command Prompt (not PowerShell)",
          "Windows · Command Prompt (PowerShell မဟုတ်ပါ)"
        ],
        "body": [
          "Find the app’s installation directory through its shortcut’s <strong>Open file location</strong>. Replace the example application path below.",
          "App shortcut ၏ <strong>Open file location</strong> မှ Install လုပ်ထားသော Folder ကို ရှာပါ။ အောက်ပါ နမူနာ App တည်နေရာကို အစားထိုးပါ။"
        ],
        "command": "\"C:\\path\\to\\Fairway XMP\\runtime\\mysql\\bin\\mysql.exe\" --defaults-file=\"%LOCALAPPDATA%\\dev.fairway.environment\\config\\admin.cnf\" --default-character-set=utf8mb4 < \"C:\\Backups\\fairway-backup.sql\""
      },
      {
        "title": [
          "Linux · default installation",
          "Linux · မူလ Installation တည်နေရာ"
        ],
        "command": "\"$HOME/.local/share/fairway-xmp/current/runtime/mysql/bin/mysql\" \\\n  --defaults-file=\"${XDG_DATA_HOME:-$HOME/.local/share}/dev.fairway.environment/config/admin.cnf\" \\\n  --default-character-set=utf8mb4 < \"/path/to/fairway-backup.sql\""
      }
    ],
    "check": [
      "The restored databases, tables and records are visible in Browse Data or phpMyAdmin. Verify any restored views, triggers and routines before discarding the backup.",
      "ပြန်ထည့်ထားသော Database၊ Table နှင့် Record များကို Browse Data သို့မဟုတ် phpMyAdmin တွင် တွေ့ရမည်။ Backup ကို မဖယ်ရှားမီ View၊ Trigger နှင့် Routine များကိုလည်း စစ်ဆေးပါ။"
    ],
    "related": [
      "import-sql"
    ],
    "intro": [
      "Requires a full SQL backup created by Back up databases and the same MySQL family that produced it, normally the current Fairway XMP installation.",
      "Back up databases မှ သိမ်းထားသော SQL Backup အပြည့်အစုံ လိုအပ်သည်။ Backup ထုတ်ခဲ့သည့် MySQL family နှင့် ကိုက်ညီသော စနစ်ကို သုံးရမည်။ ပုံမှန်အားဖြင့် လက်ရှိ Fairway XMP installation ကို သုံးနိုင်သည်။"
    ]
  },
  {
    "id": "mailpit",
    "group": "data",
    "title": [
      "Configure local email delivery",
      "အီးမေးလ်ပေးပို့မှုပိုင်း ပြင်ဆင်ရန်"
    ],
    "steps": [
      [
        "Start <strong>Mail</strong> in <strong>Environment</strong>.",
        "Open <strong>View Mail</strong> and use the displayed SMTP settings in the application’s mail configuration. The defaults are:<pre><code>Host: 127.0.0.1\nPort: 11025\nAuthentication: none\nEncryption: none</code></pre>",
        "Open <strong>View Mail</strong> to read captured messages. The browser inbox is available at <code>http://127.0.0.1:8802/</code>. PHP’s bundled <code>mail()</code> configuration already points to the local service."
      ],
      [
        "<strong>Environment</strong> တွင် <strong>Mail</strong> ကို စတင်ပါ။",
        "<strong>View Mail</strong> ကို ဖွင့်၍ ပြထားသော SMTP settings ကို App ၏ Mail configuration တွင် သတ်မှတ်ပါ။ မူလတန်ဖိုးများမှာ အောက်ပါအတိုင်း ဖြစ်သည်။<pre><code>Host: 127.0.0.1\nPort: 11025\nAuthentication: none\nEncryption: none</code></pre>",
        "ဖမ်းယူထားသော အီးမေးလ်များကို <strong>View Mail</strong> မှ ဖတ်ပါ။ Browser မှ ကြည့်ရန် <code>http://127.0.0.1:8802/</code> ကို ဖွင့်ပါ။ ပါဝင်ပြီးသား PHP ၏ <code>mail()</code> ကို Local service သို့ ပေးပို့ရန် ပြင်ဆင်ထားပြီး ဖြစ်သည်။"
      ]
    ],
    "check": [
      "Messages sent through the configured local SMTP address appear in View Mail and the browser inbox.",
      "Local SMTP သို့ ပေးပို့သော အီးမေးလ်များကို View Mail နှင့် Browser inbox တွင် တွေ့ရမည်။"
    ],
    "image": "mail.png",
    "trouble": [
      [
        [
          "No message appears",
          "အီးမေးလ် မပေါ်လာလျှင်"
        ],
        [
          "Compare your app’s SMTP port with Fairway XMP’s saved port. Check for cached configuration or a queue worker still using old settings; restart that worker after changing configuration.",
          "App ၏ SMTP Port ကို Fairway XMP တွင် သိမ်းထားသော Port နှင့် တိုက်စစ်ပါ။ Configuration cache သို့မဟုတ် Settings အဟောင်းကို ဆက်သုံးနေသော Queue worker ရှိမရှိ စစ်ပါ။ Configuration ပြင်ဆင်ပြီးလျှင် ထို Worker ကို Restart လုပ်ပါ။"
        ]
      ]
    ],
    "linux": [
      "Run <code>fairway start mail</code> and use the browser inbox at <code>http://127.0.0.1:8802/</code>. The SMTP settings are the same.",
      "<code>fairway start mail</code> ကို Run ၍ <code>http://127.0.0.1:8802/</code> ရှိ Browser inbox ကို သုံးပါ။ SMTP settings များ အတူတူဖြစ်သည်။"
    ],
    "related": [
      "ports"
    ],
    "intro": [
      "Only messages routed to Fairway XMP’s SMTP service are captured. Applications configured with external mail providers can still send real email.",
      "Fairway XMP ၏ SMTP သို့ ပေးပို့သော အီးမေးလ်များကိုသာ ဖမ်းယူသည်။ ပြင်ပ Mail provider သတ်မှတ်ထားပါက အီးမေးလ်အစစ် ဆက်လက် ပေးပို့နိုင်သည်။"
    ]
  },
  {
    "id": "composer",
    "group": "tools",
    "title": [
      "Use bundled PHP and Composer",
      "အသင့်ပါဝင်သော PHP နှင့် Composer သုံးရန်"
    ],
    "steps": [
      [
        "Open Fairway XMP’s <strong>Terminal</strong>, or press <kbd>Ctrl + &#96;</kbd>. PHP and Composer are available there even when external command-line integration is disabled.",
        "Open a terminal for the required folder through the <strong>+</strong> menu or a site’s terminal actions in <strong>Manage Sites</strong>.",
        "Run <code>php --version</code> and <code>composer --version</code> to check the bundled tools. PHP and Composer commands do not require running services."
      ],
      [
        "Fairway XMP ၏ <strong>Terminal</strong> ကို ဖွင့်ပါ သို့မဟုတ် <kbd>Ctrl + &#96;</kbd> ကို နှိပ်ပါ။ ပြင်ပ Command-line integration ပိတ်ထားလည်း PHP နှင့် Composer ကို ထိုနေရာတွင် သုံးနိုင်သည်။",
        "<strong>+</strong> Menu သို့မဟုတ် <strong>Manage Sites</strong> ရှိ Site ၏ Terminal ဖွင့်ရန် ခလုတ်များမှ လိုအပ်သော Folder အတွက် Terminal ဖွင့်ပါ။",
        "ပါဝင်ပြီးသား Tool များကို စစ်ရန် <code>php --version</code> နှင့် <code>composer --version</code> ကို Run ပါ။ PHP နှင့် Composer Command များအတွက် Service များ စတင်ထားရန် မလိုအပ်ပါ။"
      ]
    ],
    "check": [
      "Both commands report a version. The PHP version matches the selection in Fairway XMP Settings.",
      "Command နှစ်ခုလုံးက Version ကို ပြမည်။ PHP version သည် Fairway XMP Settings တွင် ရွေးထားသည်နှင့် တူညီမည်။"
    ],
    "trouble": [
      [
        [
          "Tools unavailable in an external terminal",
          "ပြင်ပ Terminal တွင် Tool များ အသုံးပြုမရလျှင်"
        ],
        [
          "Enable <strong>Settings → Command-line tools</strong> and reopen the terminal and editor. For PATH conflicts, follow <a href=\"#command-not-found\">Fix unavailable PHP or Composer commands</a>.",
          "<strong>Settings → Command-line tools</strong> ကို ဖွင့်၍ Terminal နှင့် Editor ကို ပြန်ဖွင့်ပါ။ PATH ပြဿနာအတွက် <a href=\"#command-not-found\">PHP သို့မဟုတ် Composer Command သုံးမရသည့်အခါ ဖြေရှင်းနည်း</a> ကို ကြည့်ပါ။"
        ]
      ]
    ],
    "linux": [
      "Run <code>fairway php --version</code> and <code>fairway composer --version</code>. <code>fairway shell</code> opens a shell with the bundled tools on PATH.",
      "<code>fairway php --version</code> နှင့် <code>fairway composer --version</code> ကို Run ပါ။ <code>fairway shell</code> က ပါဝင်ပြီးသား Tool များကို PATH တွင် ထည့်ထားသော Shell ကို ဖွင့်ပေးသည်။"
    ],
    "related": [
      "terminal",
      "php",
      "command-not-found"
    ]
  },
  {
    "id": "php",
    "group": "tools",
    "title": [
      "Switch the active PHP version",
      "လက်ရှိအသုံးပြုနေသော PHP version ပြောင်းရန်"
    ],
    "steps": [
      [
        "Stop all services and finish any running PHP commands or development servers.",
        "Open <strong>Settings → Service Settings → Advanced settings</strong>, select the required PHP version and choose <strong>Save</strong>.",
        "Start the services you need. Open a new terminal and run <code>php --version</code>; restart project development servers."
      ],
      [
        "Service အားလုံးကို ရပ်၍ Run နေသော PHP Command နှင့် Development server များကို အပြီးသတ်ပါ။",
        "<strong>Settings → Service Settings → Advanced settings</strong> တွင် လိုအပ်သော PHP version ကို ရွေး၍ <strong>Save</strong> ကို နှိပ်ပါ။",
        "လိုအပ်သော Service များကို ပြန်စတင်ပါ။ Terminal အသစ်တွင် <code>php --version</code> ကို Run ပါ။ ပရောဂျက်၏ Development server များကိုလည်း ပြန်စတင်ပါ။"
      ]
    ],
    "check": [
      "Settings shows the saved PHP version, and <code>php --version</code> in a new Fairway XMP terminal reports the same version.",
      "သိမ်းထားသော PHP version ကို Settings တွင် တွေ့ရမည်။ Fairway XMP Terminal အသစ်တွင် <code>php --version</code> ကို Run လျှင်လည်း ထို Version အတိုင်း ပြမည်။"
    ],
    "trouble": [
      [
        [
          "An older PHP selection blocks startup",
          "PHP version အဟောင်း ရွေးထား၍ Service စတင်မရလျှင်"
        ],
        [
          "PHP 8.0 and 8.1 are no longer bundled. Keep services stopped, choose 8.2 or later here, and save before starting again.",
          "PHP 8.0 နှင့် 8.1 မပါဝင်တော့ပါ။ Service များ ရပ်ထားစဉ် PHP 8.2 သို့မဟုတ် ပိုသစ်သော Version ကို ရွေး၍ သိမ်းဆည်းပြီးမှ ပြန်စတင်ပါ။"
        ]
      ]
    ],
    "linux": [
      "Run <code>fairway stop all</code>, then <code>fairway php-version 8.2</code> (replace <code>8.2</code> as needed). Verify with <code>fairway php --version</code>, then <code>fairway start</code>.",
      "<code>fairway stop all</code> ကို Run ပြီး <code>fairway php-version 8.2</code> ဖြင့် Version ပြောင်းပါ။ <code>8.2</code> နေရာတွင် လိုအပ်သော Version ကို သုံးပါ။ <code>fairway php --version</code> ဖြင့် စစ်၍ <code>fairway start</code> ဖြင့် ပြန်စတင်ပါ။"
    ],
    "related": [
      "settings",
      "command-not-found"
    ],
    "intro": [
      "One selection applies to all Fairway XMP sites, terminal PHP and Composer. Available versions are PHP 8.2–8.5.",
      "ရွေးထားသော PHP version တစ်ခုတည်းကို Fairway XMP Site အားလုံး၊ Terminal PHP နှင့် Composer တို့တွင် အသုံးပြုသည်။ PHP 8.2–8.5 ကို ရွေးနိုင်သည်။"
    ]
  },
  {
    "id": "settings",
    "group": "tools",
    "title": [
      "Change PHP limits or timezone",
      "PHP Settings များ ပြောင်းရန်"
    ],
    "steps": [
      [
        "Stop Nginx/PHP. MySQL and Mail can keep running.",
        "Open <strong>Settings → PHP settings</strong>. Check the displayed PHP version is the one your project uses.",
        "Adjust the relevant value: memory limit, execution time, upload size or timezone (for example <code>Asia/Yangon</code>). Save, then start the web server again. Restart running PHP commands or project development servers too."
      ],
      [
        "Nginx/PHP ကို ရပ်ပါ။ MySQL နှင့် Mail ကို ဆက်လက်အသုံးပြုနိုင်သည်။",
        "<strong>Settings → PHP settings</strong> ကို ဖွင့်ပါ။ ပြထားသော PHP version သည် ပရောဂျက်တွင် အသုံးပြုနေသော Version ဖြစ်ကြောင်း စစ်ပါ။",
        "လိုအပ်သော Memory limit၊ Execution time၊ Upload size သို့မဟုတ် Timezone ကို ပြင်ဆင်ပါ။ Timezone အတွက် <code>Asia/Yangon</code> ကဲ့သို့ တန်ဖိုးကို သုံးနိုင်သည်။ သိမ်းဆည်းပြီး Web server ကို ပြန်စတင်ပါ။ Run နေသော PHP Command နှင့် ပရောဂျက် Development server များကိုလည်း ပြန်စတင်ပါ။"
      ]
    ],
    "check": [
      "The selected version’s overrides are saved and used by new PHP processes. Overrides persist through app updates.",
      "ရွေးထားသော PHP version ၏ ပြင်ဆင်ချက်များကို သိမ်းဆည်းပြီး PHP process အသစ်များတွင် အသုံးပြုမည်။ App Update လုပ်လည်း ပြင်ဆင်ချက်များ ဆက်ရှိနေမည်။"
    ],
    "image": "settings.png",
    "trouble": [
      [
        [
          "Settings were saved for an inactive PHP version",
          "လက်ရှိအသုံးမပြုသော PHP version အတွက် Settings သိမ်းမိလျှင်"
        ],
        [
          "Editing an inactive version does not activate it. Return to the active version, or <a href=\"#php\">switch PHP</a>. <strong>Reset to defaults</strong> resets only the version being edited.",
          "လက်ရှိအသုံးမပြုသော Version ၏ Settings ကို ပြင်ဆင်ခြင်းက ထို Version ကို ရွေးသုံးပေးမည် မဟုတ်ပါ။ လက်ရှိအသုံးပြုနေသော Version ကို ပြန်ရွေးပါ သို့မဟုတ် <a href=\"#php\">PHP version ပြောင်းပါ</a>။ <strong>Reset to defaults</strong> က ပြင်ဆင်နေသော Version တစ်ခုတည်းကိုသာ မူလတန်ဖိုးများ ပြန်သတ်မှတ်ပေးသည်။"
        ]
      ],
      [
        [
          "The upload or request still fails",
          "Upload သို့မဟုတ် Request ဆက်မအောင်မြင်လျှင်"
        ],
        [
          "The upload limit is per file; multiple files must fit the total request limit. Execution time and web-server response inactivity are separate limits. Fairway XMP settings do not change limits enforced by the application. Inspect <a href=\"#logs\">Logs</a> for the reported limit.",
          "Upload limit သည် ဖိုင်တစ်ဖိုင်အတွက် ဖြစ်သည်။ ဖိုင်များစွာ ပေးပို့လျှင် စုစုပေါင်း Request limit အတွင်းလည်း ဝင်ရန် လိုအပ်သည်။ Execution time နှင့် Web server မှ တုံ့ပြန်မှုမရှိဘဲ စောင့်နိုင်သောအချိန်သည် မတူညီပါ။ Fairway XMP Settings က App ကိုယ်တိုင် သတ်မှတ်ထားသော Limit များကို မပြောင်းပေးပါ။ မည်သည့် Limit ကြောင့် ဖြစ်ကြောင်း <a href=\"#logs\">Logs</a> မှ စစ်ပါ။"
        ]
      ]
    ],
    "linux": [
      "The Linux dashboard has no PHP-settings editor. This procedure applies to the desktop app.",
      "Linux dashboard တွင် PHP settings ပြင်ဆင်ရန် နေရာ မပါဝင်ပါ။ ဤလုပ်ဆောင်နည်းသည် Desktop App အတွက် ဖြစ်သည်။"
    ]
  },
  {
    "id": "node",
    "group": "tools",
    "title": [
      "Install or switch Node.js",
      "Node.js ထည့်သွင်းရန် သို့မဟုတ် Version ပြောင်းရန်"
    ],
    "steps": [
      [
        "Open <strong>Settings → Node.js</strong> and check the detected version and source. Expand <strong>Manage Node.js</strong> if available.",
        "Choose the release required by your project. If Node is missing, choose <strong>Install & use</strong>. With an existing setup, <strong>Install</strong> downloads a version; choose <strong>Use</strong> beside that version when you want to switch.",
        "Open a new terminal and run <code>node --version</code> and <code>npm --version</code>. Restart development servers to use the new version."
      ],
      [
        "<strong>Settings → Node.js</strong> ကို ဖွင့်၍ တွေ့ရှိထားသော Version နှင့် မည်သည့်နေရာမှ အသုံးပြုနေကြောင်း စစ်ပါ။ <strong>Manage Node.js</strong> ရှိလျှင် ဖွင့်ကြည့်ပါ။",
        "ပရောဂျက်အတွက် လိုအပ်သော Version ကို ရွေးပါ။ Node မရှိသေးလျှင် <strong>Install & use</strong> ကို နှိပ်ပါ။ ရှိပြီးသား Node setup ရှိလျှင် <strong>Install</strong> က Version အသစ်ကို Download လုပ်ပေးသည်။ ပြောင်းသုံးလိုလျှင် ထို Version ဘေးရှိ <strong>Use</strong> ကို နှိပ်ပါ။",
        "Terminal အသစ်တွင် <code>node --version</code> နှင့် <code>npm --version</code> ကို Run ပါ။ Version အသစ်ဖြင့် အသုံးပြုရန် Development server များကို ပြန်စတင်ပါ။"
      ]
    ],
    "check": [
      "New terminals use the selected version. External-terminal activation also requires <strong>Settings → Command-line tools</strong> to be enabled.",
      "Terminal အသစ်များတွင် ရွေးထားသော Version ကို အသုံးပြုမည်။ ပြင်ပ Terminal တွင်လည်း အသုံးပြုရန် <strong>Settings → Command-line tools</strong> ကို ဖွင့်ထားရန် လိုအပ်သည်။"
    ],
    "trouble": [
      [
        [
          "Node settings are read-only",
          "Node settings ကို ပြင်ဆင်မရလျှင်"
        ],
        [
          "Fairway XMP found an existing fnm installation or populated fnm store. Use the commands under <strong>Manage in your terminal</strong> with your existing setup.",
          "Fairway XMP က ရှိပြီးသား fnm installation သို့မဟုတ် Node version များ သိမ်းထားသော fnm store ကို တွေ့ရှိထားခြင်း ဖြစ်သည်။ <strong>Manage in your terminal</strong> အောက်ရှိ Command များဖြင့် လက်ရှိ Setup မှ ဆက်လက်စီမံပါ။"
        ]
      ],
      [
        [
          "Restore the previous Node setup",
          "ယခင် Node setup ကို ပြန်သုံးရန်"
        ],
        [
          "Choose <strong>Use existing Node setup</strong> and open a new terminal. Other managers’ installations are kept. Uninstalling an fnm version removes that version’s global npm packages too.",
          "<strong>Use existing Node setup</strong> ကို ရွေး၍ Terminal အသစ် ဖွင့်ပါ။ အခြား Manager များဖြင့် ထည့်သွင်းထားသော Node များ ဆက်ရှိနေမည်။ fnm မှ Version တစ်ခုကို ဖယ်ရှားလျှင် ထို Version ၏ Global npm package များပါ ဖယ်ရှားမည်။"
        ]
      ]
    ],
    "linux": [
      "The Linux edition includes the fnm CLI but has no Node.js settings page or automatic Node activation. Node versions and shell initialization are managed through fnm or an existing Node setup. See <a href=\"https://github.com/eimg/fairway-xmp/blob/main/docs/node-tools.md\">Node tools reference (English)</a> for activation details.",
      "Linux edition တွင် fnm CLI ပါဝင်သော်လည်း Node.js Settings စာမျက်နှာနှင့် အလိုအလျောက် Node activation မပါဝင်ပါ။ Node version နှင့် Shell initialization ကို fnm သို့မဟုတ် ရှိပြီးသား Node setup မှ စီမံပါ။ Activation အသေးစိတ်အတွက် <a href=\"https://github.com/eimg/fairway-xmp/blob/main/docs/node-tools.md\">Node tools လမ်းညွှန် (English)</a> ကို ကြည့်ပါ။"
    ],
    "related": [
      "terminal",
      "command-not-found"
    ]
  },
  {
    "id": "terminal",
    "group": "tools",
    "title": [
      "Open terminals for project folders",
      "ပရောဂျက် Folder အတွက် Terminal ဖွင့်ရန်"
    ],
    "steps": [
      [
        "Open <strong>Terminal</strong> or press <kbd>Ctrl + &#96;</kbd>. The first tab starts in <code>fairway/www</code>.",
        "Choose <strong>+</strong> beside the tabs and select a project or registered site folder for a second terminal.",
        "For a saved site, open its actions menu in <strong>Manage Sites</strong> and choose <strong>Terminal in web root</strong>. When the web root is named <code>public</code>, <strong>Terminal in app folder</strong> opens its parent. A matching tab is focused; otherwise a free tab is used."
      ],
      [
        "<strong>Terminal</strong> ကို ဖွင့်ပါ သို့မဟုတ် <kbd>Ctrl + &#96;</kbd> ကို နှိပ်ပါ။ ပထမ Tab သည် <code>fairway/www</code> တွင် စတင်သည်။",
        "ဒုတိယ Terminal ဖွင့်ရန် Tab ဘေးရှိ <strong>+</strong> ကို နှိပ်၍ ပရောဂျက် သို့မဟုတ် သတ်မှတ်ထားသော Site ၏ Folder ကို ရွေးပါ။",
        "သိမ်းထားသော Site အတွက် <strong>Manage Sites</strong> ရှိ Actions menu မှ <strong>Terminal in web root</strong> ကို ရွေးပါ။ Web root အမည်သည် <code>public</code> ဖြစ်လျှင် <strong>Terminal in app folder</strong> က ၎င်း၏ အပေါ်တစ်ဆင့် Folder ကို ဖွင့်ပေးသည်။ တူညီသောနေရာတွင် Tab ဖွင့်ထားပြီးလျှင် ထို Tab သို့ သွားပေးပြီး မရှိလျှင် အားနေသော Tab တွင် ဖွင့်ပေးသည်။"
      ]
    ],
    "check": [
      "The terminal opens in the selected folder. Tabs and running commands remain active when navigating between pages or hiding the window.",
      "ရွေးထားသော Folder တွင် Terminal ဖွင့်လာမည်။ စာမျက်နှာပြောင်းခြင်း သို့မဟုတ် Window ဖျောက်ထားခြင်းတွင် Tab နှင့် Run နေသော Command များ ဆက်လက်အလုပ်လုပ်နေမည်။"
    ],
    "image": "terminal.png",
    "trouble": [
      [
        [
          "Both tabs are already occupied",
          "Tab နှစ်ခုလုံး အသုံးပြုနေပြီးဖြစ်လျှင်"
        ],
        [
          "Close a tab you no longer need before opening another. <strong>External terminal</strong> opens your normal terminal at the active shell’s current folder and stays open independently of Fairway XMP.",
          "နောက်ထပ် Terminal မဖွင့်မီ မလိုတော့သော Tab တစ်ခုကို ပိတ်ပါ။ <strong>External terminal</strong> က လက်ရှိ Shell အသုံးပြုနေသော Folder တွင် ပြင်ပ Terminal ကို ဖွင့်ပေးသည်။ Fairway XMP နှင့် သီးခြားဖြစ်၍ App ပိတ်လည်း ဆက်ရှိနေမည်။"
        ]
      ],
      [
        [
          "Stop or restart a terminal session",
          "Terminal session ကို ရပ်ရန် သို့မဟုတ် ပြန်စတင်ရန်"
        ],
        [
          "Use <kbd>Ctrl + C</kbd> for the running command. <strong>Restart terminal</strong> ends that tab’s commands and opens a fresh shell. Quitting Fairway XMP, installing an update or reloading the interface closes embedded sessions.",
          "Run နေသော Command ကို ရပ်ရန် <kbd>Ctrl + C</kbd> ကို နှိပ်ပါ။ <strong>Restart terminal</strong> က ထို Tab ၏ Command များကို ရပ်၍ Shell အသစ် ဖွင့်ပေးသည်။ Fairway XMP မှ ထွက်ခြင်း၊ Update ထည့်သွင်းခြင်း သို့မဟုတ် Interface ကို Reload လုပ်ခြင်းက App အတွင်းရှိ Terminal session များကို ပိတ်သည်။"
        ]
      ]
    ],
    "linux": [
      "Use separate tabs in your normal terminal. Run <code>fairway shell</code> in each one when you need Fairway XMP’s PHP and Composer.",
      "မိမိအသုံးပြုနေသော Terminal တွင် Tab များ သီးခြားဖွင့်ပါ။ Fairway XMP ၏ PHP နှင့် Composer ကို သုံးရန် Tab တစ်ခုစီတွင် <code>fairway shell</code> ကို Run ပါ။"
    ],
    "intro": [
      "The embedded Terminal is available on Windows and macOS and supports two independent tabs.",
      "App အတွင်းရှိ Terminal ကို Windows နှင့် macOS တွင် သုံးနိုင်သည်။ သီးခြားအလုပ်လုပ်သော Tab နှစ်ခုအထိ ဖွင့်နိုင်သည်။"
    ]
  },
  {
    "id": "site-not-opening",
    "group": "fix",
    "title": [
      "Fix a site that won’t open",
      "Site ဖွင့်မရသည့်အခါ ဖြေရှင်းရန်"
    ],
    "steps": [
      [
        "Check the web server is running. In <strong>Manage Sites</strong>, choose <strong>Start web server</strong> if it is stopped.",
        "Use the saved site’s <strong>Open</strong> action or <strong>Copy URL</strong>. Include the port, normally <code>:8800</code> for HTTP or <code>:8443</code> for HTTPS.",
        "If the app shows <strong>Folder missing</strong>, restore the folder or edit the site’s web root to its new location. If address setup is pending, complete it using <strong>Set up addresses</strong> or <strong>Check approval</strong>.",
        "If the server is running but the page reports an error, inspect <a href=\"#logs\">service and PHP error logs</a>."
      ],
      [
        "Web server စတင်ထားကြောင်း စစ်ပါ။ ရပ်နေလျှင် <strong>Manage Sites</strong> ရှိ <strong>Start web server</strong> ကို နှိပ်ပါ။",
        "သိမ်းထားသော Site ၏ <strong>Open</strong> သို့မဟုတ် <strong>Copy URL</strong> ကို သုံးပါ။ HTTP အတွက် မူလ Port <code>:8800</code>၊ HTTPS အတွက် <code>:8443</code> ကို လိပ်စာနှင့်အတူ ထည့်ပါ။",
        "<strong>Folder missing</strong> ပြနေလျှင် Folder ကို မူလနေရာသို့ ပြန်ထားပါ သို့မဟုတ် Site ၏ Web root ကို နေရာအသစ်သို့ ပြင်ဆင်ပါ။ လိပ်စာသတ်မှတ်မှု မပြီးသေးလျှင် <strong>Set up addresses</strong> သို့မဟုတ် <strong>Check approval</strong> ဖြင့် အပြီးသတ်ပါ။",
        "Web server စတင်ထားသော်လည်း စာမျက်နှာတွင် Error ပြနေလျှင် <a href=\"#logs\">Service နှင့် PHP Error log များ</a>ကို စစ်ပါ။"
      ]
    ],
    "check": [
      "The site status is ready and its saved URL opens the selected web root. Moved folders and pending approval can be corrected on the existing registration.",
      "Site အသင့်ဖြစ်ကြောင်း ပြသ၍ သိမ်းထားသော လိပ်စာမှ Web root ကို ဖွင့်နိုင်မည်။ Folder ရွှေ့ထားခြင်းနှင့် ခွင့်ပြုချက်စောင့်ဆိုင်းနေခြင်းတို့ကို ရှိပြီးသား Site တွင် ပြင်ဆင်နိုင်သည်။"
    ],
    "trouble": [
      [
        [
          "Laravel’s home page works but other routes return 404",
          "Laravel Home page ရသော်လည်း အခြား Route များတွင် 404 ပြနေလျှင်"
        ],
        [
          "Edit the site: choose the <code>public</code> folder as web root and <strong>Laravel / front controller</strong> as app type. Save and retry the route.",
          "Site ကို ပြင်ဆင်၍ Web root အဖြစ် <code>public</code> Folder၊ App type အဖြစ် <strong>Laravel / front controller</strong> ကို ရွေးပါ။ သိမ်းဆည်းပြီး Route ကို ပြန်ဖွင့်ပါ။"
        ]
      ],
      [
        [
          "Services won’t start because a port is in use",
          "Port ညိနေ၍ Service စတင်မရလျှင်"
        ],
        [
          "Follow <a href=\"#ports\">Resolve a port conflict</a>, then open the updated URL shown in Fairway XMP.",
          "<a href=\"#ports\">Port ညိသည့်အခါ ဖြေရှင်းနည်း</a> အတိုင်း လုပ်ဆောင်၍ Fairway XMP တွင် ပြထားသော လိပ်စာအသစ်ကို ဖွင့်ပါ။"
        ]
      ]
    ],
    "linux": [
      "Use <code>fairway status</code> to check services and sites. Run <code>fairway site hosts</code> if the .test address needs setup again.",
      "Service နှင့် Site များ၏ အခြေအနေကို <code>fairway status</code> ဖြင့် ကြည့်ပါ။ .test လိပ်စာသတ်မှတ်မှု ပြန်လုပ်ရန် လိုအပ်လျှင် <code>fairway site hosts</code> ကို Run ပါ။"
    ],
    "related": [
      "https",
      "logs",
      "ports"
    ]
  },
  {
    "id": "logs",
    "group": "fix",
    "title": [
      "Read service and PHP error logs",
      "Error အပါအဝင် Log များ ကြည့်ရန်"
    ],
    "steps": [
      [
        "Open <strong>Logs</strong>, then reproduce the failed request or service action.",
        "Locate the request entry. For PHP errors, select <strong>Shared PHP</strong> for the file, line number and error message. Shared PHP entries can come from any site, including when a site filter is selected.",
        "Use <strong>Copy shown</strong> to copy the filtered entries, or <strong>Open log folder</strong> to inspect the underlying files. Remove passwords and private data before sharing copied logs."
      ],
      [
        "<strong>Logs</strong> ကို ဖွင့်၍ Error ဖြစ်သော Request သို့မဟုတ် Service လုပ်ဆောင်ချက်ကို ပြန်လုပ်ပါ။",
        "Request မှတ်တမ်းကို ရှာပါ။ PHP Error အတွက် <strong>Shared PHP</strong> မှ ဖိုင်အမည်၊ Line number နှင့် Error message ကို ကြည့်ပါ။ Site filter ရွေးထားလည်း Shared PHP မှတ်တမ်းများသည် မည်သည့် Site မှမဆို ဖြစ်နိုင်သည်။",
        "ပြထားသော မှတ်တမ်းများကို ကူးယူရန် <strong>Copy shown</strong>၊ မူလ Log ဖိုင်များ ကြည့်ရန် <strong>Open log folder</strong> ကို သုံးပါ။ Log များ မျှဝေမပေးပို့မီ Password နှင့် ထိရှလွယ်အချက်အလက်များကို ဖယ်ရှားပါ။"
      ]
    ],
    "check": [
      "Recent entries from the selected source are visible. Copy shown includes only the entries in the current filtered view.",
      "ရွေးထားသော Source ၏ နောက်ဆုံးမှတ်တမ်းများကို တွေ့ရမည်။ Copy shown သည် လက်ရှိ Filter အရ ပြထားသော မှတ်တမ်းများကိုသာ ကူးယူပေးသည်။"
    ],
    "image": "logs.png",
    "trouble": [
      [
        [
          "Expected entries are missing",
          "လိုအပ်သော မှတ်တမ်းများ မတွေ့ရလျှင်"
        ],
        [
          "Turn off <strong>Problems only</strong> to include successful requests and resume the viewer if paused. Application logs and terminal output are outside this viewer. <strong>Clear view</strong> hides current entries without deleting log files; <strong>Open log folder</strong> opens those files.",
          "အောင်မြင်သော Request များပါ ကြည့်ရန် <strong>Problems only</strong> ကို ပိတ်ပါ။ Viewer ကို ခေတ္တရပ်ထားလျှင် ပြန်စတင်ပါ။ App ၏ သီးခြား Log များနှင့် Terminal output များသည် ဤနေရာတွင် မပါဝင်ပါ။ <strong>Clear view</strong> က Log ဖိုင်များကို မဖျက်ဘဲ လက်ရှိမှတ်တမ်းများကိုသာ ဖျောက်ထားသည်။ မူလဖိုင်များကို <strong>Open log folder</strong> မှ ဖွင့်နိုင်သည်။"
        ]
      ]
    ],
    "linux": [
      "Read the relevant output with <code>fairway logs nginx</code>, <code>fairway logs php</code>, <code>fairway logs mysql</code> or <code>fairway logs mail</code>.",
      "သက်ဆိုင်ရာ Log များကို <code>fairway logs nginx</code>၊ <code>fairway logs php</code>၊ <code>fairway logs mysql</code> သို့မဟုတ် <code>fairway logs mail</code> ဖြင့် ကြည့်နိုင်သည်။"
    ]
  },
  {
    "id": "command-not-found",
    "group": "fix",
    "title": [
      "Fix unavailable PHP or Composer commands",
      "PHP သို့မဟုတ် Composer Command သုံးမရသည့်အခါ ဖြေရှင်းရန်"
    ],
    "steps": [
      [
        "On Windows or macOS, open <strong>Settings → Command-line tools</strong> and enable the integration.",
        "Close and reopen your external terminal and editor so they read the updated PATH.",
        "Run <code>php --version</code> and <code>composer --version</code>. To work immediately inside Fairway XMP, open its embedded <strong>Terminal</strong>; PHP and Composer are available there even with integration off."
      ],
      [
        "Windows သို့မဟုတ် macOS တွင် <strong>Settings → Command-line tools</strong> မှ Command-line integration ကို ဖွင့်ပါ။",
        "ပြင်ဆင်ထားသော PATH ကို ရရှိရန် ပြင်ပ Terminal နှင့် Editor ကို ပိတ်၍ ပြန်ဖွင့်ပါ။",
        "<code>php --version</code> နှင့် <code>composer --version</code> ကို Run ပါ။ Fairway XMP အတွင်းရှိ <strong>Terminal</strong> တွင်လည်း သုံးနိုင်သည်။ Command-line integration ပိတ်ထားလည်း ထိုနေရာတွင် PHP နှင့် Composer ကို အသုံးပြုနိုင်သည်။"
      ]
    ],
    "check": [
      "Both commands print a version. PHP matches the version selected in Fairway XMP Settings.",
      "Command နှစ်ခုလုံးက Version ကို ပြမည်။ PHP version သည် Fairway XMP Settings တွင် ရွေးထားသည်နှင့် တူညီမည်။"
    ],
    "trouble": [
      [
        [
          "Another PHP installation still takes priority",
          "အခြားနေရာမှ PHP ကို ဆက်အသုံးပြုနေလျှင်"
        ],
        [
          "Check the executable path with <code>where php</code> in Windows Command Prompt or <code>command -v php</code> on macOS/Linux. Compare it with <strong>Command details</strong> in Settings. Check your shell’s PATH or use the embedded Terminal for Fairway XMP’s selected PHP.",
          "Command ဖိုင်တည်နေရာကို Windows Command Prompt တွင် <code>where php</code>၊ macOS/Linux တွင် <code>command -v php</code> ဖြင့် ကြည့်ပါ။ Settings ရှိ <strong>Command details</strong> နှင့် တိုက်စစ်ပါ။ Shell ၏ PATH ကို စစ်ဆေးပါ သို့မဟုတ် Fairway XMP ရွေးထားသော PHP ကို သုံးရန် App အတွင်းရှိ Terminal ကို သုံးပါ။"
        ]
      ]
    ],
    "linux": [
      "Reopen your shell after installation. Use <code>fairway shell</code> or invoke <code>fairway php --version</code> and <code>fairway composer --version</code> directly. Fish users can use the absolute Fairway XMP path shown by the installer or add it to their PATH.",
      "Install လုပ်ပြီးလျှင် Shell ကို ပြန်ဖွင့်ပါ။ <code>fairway shell</code> ကို သုံးပါ သို့မဟုတ် <code>fairway php --version</code> နှင့် <code>fairway composer --version</code> ကို တိုက်ရိုက် Run ပါ။ Fish အသုံးပြုသူများသည် Installer ပြထားသော Fairway XMP Command ၏ တည်နေရာအပြည့်အစုံကို သုံးနိုင်သည် သို့မဟုတ် PATH ထဲ ထည့်နိုင်သည်။"
    ],
    "related": [
      "php",
      "composer"
    ]
  },
  {
    "id": "ports",
    "group": "fix",
    "title": [
      "Resolve a port conflict",
      "Port ညိသည့်အခါ ဖြေရှင်းရန်"
    ],
    "steps": [
      [
        "Note the conflicting service and port from the error, then stop all Fairway XMP services.",
        "Open <strong>Settings → Service Settings</strong>, choose a free port for that service and save. Use the suggested alternative if one is shown.",
        "Start services again. Use Fairway XMP’s updated URLs; if you changed MySQL or SMTP, update the port in your project’s <code>.env</code> or connection settings too. Clear cached configuration or restart workers as needed."
      ],
      [
        "Error တွင် ပြထားသော Service နှင့် Port ကို မှတ်၍ Fairway XMP Service အားလုံးကို ရပ်ပါ။",
        "<strong>Settings → Service Settings</strong> တွင် ထို Service အတွက် အသုံးမပြုရသေးသော Port ကို ရွေး၍ သိမ်းပါ။ အစားထိုး Port အကြံပြုထားလျှင် ၎င်းကို သုံးနိုင်သည်။",
        "Service များကို ပြန်စတင်၍ Fairway XMP ၏ လိပ်စာအသစ်များကို သုံးပါ။ MySQL သို့မဟုတ် SMTP Port ပြောင်းထားလျှင် ပရောဂျက်၏ <code>.env</code> သို့မဟုတ် Connection settings တွင်လည်း ပြင်ဆင်ပါ။ လိုအပ်လျှင် Configuration cache ကို ရှင်းပါ သို့မဟုတ် Worker များကို ပြန်စတင်ပါ။"
      ]
    ],
    "check": [
      "The service starts on the saved port without a conflict. Environment and the saved site URLs show the updated values.",
      "Service သည် သိမ်းထားသော Port တွင် ပြဿနာမရှိဘဲ စတင်နိုင်မည်။ Environment နှင့် Site လိပ်စာများတွင် တန်ဖိုးအသစ်များကို ပြထားမည်။"
    ],
    "linux": [
      "Run <code>fairway ports</code> to see saved values, then <code>fairway stop all</code>. Set all six values in this order: <code>fairway ports WEB PMA DB PHP MAIL SMTP</code>. For example, <code>fairway ports 8803 8801 13306 19000 8802 11025</code> changes only the website port when the others use their defaults. Keep your other saved ports if they differ, then run <code>fairway start</code>.",
      "လက်ရှိ Port များကို <code>fairway ports</code> ဖြင့် ကြည့်၍ <code>fairway stop all</code> ဖြင့် Service အားလုံးကို ရပ်ပါ။ တန်ဖိုးခြောက်ခုလုံးကို <code>fairway ports WEB PMA DB PHP MAIL SMTP</code> အစဉ်အတိုင်း ထည့်ပါ။ ဥပမာ အခြား Port များ မူလအတိုင်း ဖြစ်လျှင် <code>fairway ports 8803 8801 13306 19000 8802 11025</code> က Website Port တစ်ခုတည်းကို ပြောင်းပေးသည်။ အခြား Port များ ပြင်ထားပြီးဖြစ်လျှင် ထိုတန်ဖိုးများကို ဆက်သုံးပါ။ ပြီးလျှင် <code>fairway start</code> ဖြင့် ပြန်စတင်ပါ။"
    ],
    "related": [
      "default-ports",
      "mysql",
      "mailpit"
    ]
  },
  {
    "id": "lifecycle",
    "group": "management",
    "title": [
      "Stop services or quit Fairway XMP",
      "Service များ ရပ်ရန် သို့မဟုတ် Fairway XMP မှ ထွက်ရန်"
    ],
    "steps": [
      [
        "Choose <strong>Stop all</strong> to stop services while keeping Fairway XMP open. Individual Nginx, MySQL and Mail controls stop only that service; stopping Nginx also stops its PHP worker.",
        "Choose <strong>Quit</strong> from the macOS menu bar or Windows system tray menu to stop services, close embedded terminals and exit Fairway XMP.",
        "Closing or minimizing the window keeps Fairway XMP and its services running. Use the menu bar or tray icon to reopen the panel."
      ],
      [
        "Fairway XMP ကို ဖွင့်ထားပြီး Service များ ရပ်ရန် <strong>Stop all</strong> ကို ရွေးပါ။ Nginx၊ MySQL နှင့် Mail တစ်ခုစီကိုလည်း သီးခြား ရပ်နိုင်သည်။ Nginx ကို ရပ်လျှင် PHP worker ပါ ရပ်သည်။",
        "Service နှင့် App အတွင်းရှိ Terminal များကို ရပ်၍ Fairway XMP မှ ထွက်ရန် macOS Menu bar သို့မဟုတ် Windows System tray menu မှ <strong>Quit</strong> ကို ရွေးပါ။",
        "Window ကို ပိတ်ခြင်း သို့မဟုတ် Minimize လုပ်ခြင်းတွင် Fairway XMP နှင့် Service များ ဆက်လက်အလုပ်လုပ်နေသည်။ Panel ကို Menu bar သို့မဟုတ် System tray icon မှ ပြန်ဖွင့်နိုင်သည်။"
      ]
    ],
    "check": [
      "Stop all leaves services stopped in the panel. Quit also exits Fairway XMP. Project files and saved database data remain on disk.",
      "Stop all လုပ်လျှင် Panel တွင် Service များ ရပ်ထားမည်။ Quit လုပ်လျှင် Fairway XMP မှပါ ထွက်မည်။ ပရောဂျက်ဖိုင်နှင့် သိမ်းထားသော Database data များ ဆက်ရှိနေမည်။"
    ],
    "linux": [
      "In the dashboard, lowercase <code>q</code> closes the dashboard and leaves services running. Uppercase <code>Q</code> or <code>fairway shutdown</code> stops services and the supervisor. Use <code>fairway stop all</code> to stop services without shutting down the supervisor.",
      "Dashboard တွင် စာလုံးအသေး <code>q</code> က Dashboard ကိုသာ ပိတ်ပြီး Service များ ဆက်လက်အလုပ်လုပ်နေမည်။ စာလုံးအကြီး <code>Q</code> သို့မဟုတ် <code>fairway shutdown</code> က Service နှင့် Supervisor ကို ရပ်သည်။ Supervisor မပိတ်ဘဲ Service များကိုသာ ရပ်ရန် <code>fairway stop all</code> ကို သုံးပါ။"
    ]
  },
  {
    "id": "updates",
    "group": "management",
    "title": [
      "Update Fairway XMP",
      "Fairway XMP ကို Update လုပ်ရန်"
    ],
    "steps": [
      [
        "Save pending work and <a href=\"#backups\">back up databases</a> before installation.",
        "Open <strong>Updates → Check for updates</strong>, review the release notes and choose <strong>Install update</strong>.",
        "Wait for download and signature verification. Services remain available during this stage. Before installation, Fairway XMP backs up MySQL, stops services and embedded terminals, then restarts."
      ],
      [
        "Update မထည့်သွင်းမီ လုပ်လက်စအလုပ်များကို သိမ်း၍ <a href=\"#backups\">Database များကို Backup သိမ်းပါ</a>။",
        "<strong>Updates → Check for updates</strong> ကို ဖွင့်၍ ဖြန့်ဝေမှုမှတ်ချက်များကို ဖတ်ပြီး <strong>Install update</strong> ကို ရွေးပါ။",
        "Download နှင့် Signature စစ်ဆေးမှု ပြီးသည်အထိ စောင့်ပါ။ ဤအဆင့်တွင် Service များ ဆက်လက်အသုံးပြုနိုင်သည်။ မထည့်သွင်းမီ Fairway XMP က MySQL ကို Backup သိမ်း၍ Service နှင့် App အတွင်းရှိ Terminal များကို ရပ်ပြီးနောက် Update လုပ်ကာ ပြန်ဖွင့်သည်။"
      ]
    ],
    "check": [
      "Fairway XMP reopens with the installed version shown. Saved sites and settings remain available.",
      "Fairway XMP ပြန်ဖွင့်လာ၍ ထည့်သွင်းထားသော Version ကို ပြမည်။ သိမ်းထားသော Site နှင့် Settings များ ဆက်လက်အသုံးပြုနိုင်မည်။"
    ],
    "image": "updates.png",
    "trouble": [
      [
        [
          "The app offers View downloads instead",
          "View downloads ကိုသာ ပြနေလျှင်"
        ],
        [
          "Download the newer installer. Back up databases, stop all services and quit Fairway XMP before running it. Installations without an Updates page also use this manual process.",
          "Installer အသစ်ကို Download လုပ်ပါ။ Database များ Backup သိမ်း၍ Service အားလုံးကို ရပ်ပြီး Fairway XMP မှ ထွက်ပါ။ ထို့နောက် Installer ကို Run ပါ။ Updates စာမျက်နှာ မရှိသော Installation များတွင်လည်း ဤနည်းဖြင့် Update လုပ်နိုင်သည်။"
        ]
      ],
      [
        [
          "Backup or shutdown failed",
          "Backup သိမ်းခြင်း သို့မဟုတ် Service ရပ်ခြင်း မအောင်မြင်လျှင်"
        ],
        [
          "Installation stops before replacing the app. Read the reported error, resolve it and retry. A failed download leaves the current services available.",
          "App ကို အစားမထိုးမီ Update ထည့်သွင်းမှု ရပ်သွားမည်။ ပြထားသော Error ကို ဖတ်၍ ဖြေရှင်းပြီး ပြန်ကြိုးစားပါ။ Download မအောင်မြင်လျှင် လက်ရှိ Service များကို ဆက်သုံးနိုင်သည်။"
        ]
      ]
    ],
    "linux": [
      "Download the newer <code>.run</code> installer and run it over the existing installation. Reuse the same <code>--prefix</code> if you chose a custom directory. It backs up databases and stops services before switching versions, while preserving projects and data.",
      "<code>.run</code> Installer အသစ်ကို Download လုပ်၍ ရှိပြီးသား Installation အတွက် Run ပါ။ ကိုယ်ပိုင်တည်နေရာ သတ်မှတ်ထားလျှင် မူလ <code>--prefix</code> ကို ပြန်သုံးပါ။ Installer က Version မပြောင်းမီ Database များ Backup သိမ်း၍ Service များကို ရပ်ပေးသည်။ ပရောဂျက်နှင့် ဒေတာများ ဆက်ရှိနေမည်။"
    ],
    "related": [
      "backups"
    ],
    "intro": [
      "Updates preserve project files, databases, saved sites, settings and installed Node versions. Installation closes embedded terminal sessions.",
      "Update လုပ်ရာတွင် ပရောဂျက်ဖိုင်၊ Database၊ သိမ်းထားသော Site၊ Settings နှင့် ထည့်သွင်းထားသော Node version များ ဆက်ရှိနေမည်။ Update ထည့်သွင်းချိန်တွင် App အတွင်းရှိ Terminal session များ ပိတ်သွားမည်။"
    ]
  },
  {
    "id": "uninstall",
    "group": "management",
    "title": [
      "Uninstall Fairway XMP",
      "Fairway XMP ကို ဖယ်ရှားရန်"
    ],
    "steps": [
      [
        "<a href=\"#backups\">Back up databases</a>, then quit Fairway XMP.",
        "On Windows, open <strong>Settings → Apps → Fairway XMP → Uninstall</strong>.",
        "On macOS, open <strong>Applications</strong> in Finder. Right-click <strong>Fairway XMP → Show Package Contents</strong>, open <strong>Contents → Resources</strong>, and run <strong>Uninstall Fairway XMP.command</strong>."
      ],
      [
        "<a href=\"#backups\">Database များ Backup သိမ်း</a>၍ Fairway XMP မှ ထွက်ပါ။",
        "Windows တွင် <strong>Settings → Apps → Fairway XMP → Uninstall</strong> ကို ဖွင့်ပါ။",
        "macOS တွင် Finder ၏ <strong>Applications</strong> ကို ဖွင့်ပါ။ <strong>Fairway XMP → Show Package Contents</strong> ကို Right-click နှိပ်၍ <strong>Contents → Resources</strong> သို့ ဝင်ပြီး <strong>Uninstall Fairway XMP.command</strong> ကို Run ပါ။"
      ]
    ],
    "check": [
      "The app, command-line integration, local site addresses and certificate trust are removed. Project files and database data are kept unless explicitly deleted.",
      "App၊ Command-line integration၊ Local site လိပ်စာနှင့် Certificate ယုံကြည်မှုတို့ကို ဖယ်ရှားပြီး ဖြစ်မည်။ သီးခြားဖျက်ရန် မရွေးလျှင် ပရောဂျက်ဖိုင်နှင့် Database data များ ဆက်ရှိနေမည်။"
    ],
    "linux": [
      "Run <code>fairway uninstall</code>. Projects, databases, backups and installed Node versions are kept.",
      "<code>fairway uninstall</code> ကို Run ပါ။ ပရောဂျက်၊ Database၊ Backup နှင့် ထည့်သွင်းထားသော Node version များ ဆက်ရှိနေမည်။"
    ],
    "related": [
      "restore"
    ]
  }
];
