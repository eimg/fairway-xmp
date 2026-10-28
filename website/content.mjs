// Shared interface copy, paired with the task guides in guides.mjs.
export const copy = {
  "en": {
    "lang": "en",
    "label": "English",
    "skip": "Skip to content",
    "download": "Get Fairway XMP",
    "downloadDialog": {
      "close": "Close downloads",
      "version": "Version",
      "intro": "Installers for supported platforms.",
      "detectedIntro": "Installer and requirements for the detected platform.",
      "suggested": "Detected platform",
      "other": "Other platforms",
      "choose": "Available platforms",
      "button": "Download for",
      "releaseNotes": "Release notes & all downloads",
      "requirements": {
        "windows": "Windows 10 22H2 / Windows 11 · x64",
        "macos": "macOS 15 or later · Apple Silicon only",
        "linux": "Linux x86_64 · glibc 2.35+ · Terminal edition"
      }
    },
    "footer": "A local environment for learning and building with PHP.",
    "source": "Source & releases",
    "portHeaders": [
      "Tool / endpoint",
      "Default",
      "Used for"
    ],
    "portRows": [
      [
        "Website",
        "8800",
        "HTTP projects"
      ],
      [
        "Site HTTPS",
        "8443",
        "Optional secure .test sites"
      ],
      [
        "phpMyAdmin",
        "8801",
        "Browser database interface"
      ],
      [
        "MySQL",
        "13306",
        "Application database connections"
      ],
      [
        "PHP worker",
        "19000",
        "Internal Nginx → PHP connection"
      ],
      [
        "Mail inbox",
        "8802",
        "Mailpit browser inbox"
      ],
      [
        "Mail SMTP",
        "11025",
        "Application email testing"
      ]
    ],
    "description": "Fairway XMP guides for serving project folders, configuring bundled tools, managing databases and mail, and resolving service issues.",
    "contents": "Guides",
    "overview": "How-to guides",
    "intro": "Instructions for configuring and using Fairway XMP.",
    "scopeNote": "Desktop steps apply to Windows and macOS. Linux commands are in expandable notes. Example addresses use default ports; saved values are shown in Fairway XMP.",
    "popular": "Common tasks",
    "stuck": "Troubleshooting",
    "quickTasks": [
      "sites",
      "mysql",
      "php",
      "mailpit",
      "backups",
      "node"
    ],
    "quickFixes": [
      "site-not-opening",
      "command-not-found",
      "ports",
      "logs"
    ],
    "check": "Expected result",
    "troubleshooting": "Troubleshooting",
    "related": "Related guides",
    "screenshotLabel": "Screenshot",
    "screenshot": "Desktop interface preview with sample data; shown for orientation, not a live environment.",
    "linuxTitle": "On Linux",
    "ports": "Connection details and default ports",
    "portNote": "Default values for a fresh installation are listed below. Saved connection details appear in Environment and Service Settings, or in fairway ports on Linux. Services bind to 127.0.0.1 and accept connections from the local computer only.",
    "portHelp": "<a href=\"#ports\">Resolve a port conflict</a> · <a href=\"#mysql\">MySQL connection settings</a>",
    "nextTitle": "Report a problem",
    "nextText": "Include the operating system, Fairway XMP version, steps to reproduce, expected result and exact error in a GitHub issue. Remove passwords and private data from any attached logs.",
    "help": "Open a GitHub issue",
    "fullDocs": "Technical guides (English)",
    "nav": {
      "docs": "How-to guides",
      "github": "GitHub"
    },
    "scope": "Fairway XMP how-to guides",
    "theme": {
      "label": "Dark mode",
      "toDark": "Switch to dark mode",
      "toLight": "Switch to light mode"
    }
  },
  "mm": {
    "lang": "my",
    "label": "မြန်မာ",
    "skip": "လမ်းညွှန်ချက်သို့ သွားရန်",
    "download": "Fairway XMP ရယူရန်",
    "downloadDialog": {
      "close": "Download ပိတ်ရန်",
      "version": "Version",
      "intro": "Platform အသီးသိးအတွက် Installer များ",
      "detectedIntro": "သင်၏ကွန်ပျူတာအတွက် Installer နှင့် System Requirements",
      "suggested": "သင်၏ကွန်ပျူတာ",
      "other": "အခြား Platform များ",
      "choose": "ရရှိနိုင်သော Platform များ",
      "button": "Download ရယူရန် ·",
      "releaseNotes": "ဖြန့်ဝေမှုမှတ်ချက် နှင့် Download အားလုံး",
      "requirements": {
        "windows": "Windows 10 22H2 / Windows 11 · x64",
        "macos": "macOS 15 နှင့်အထက် · Apple Silicon သီးသန့်",
        "linux": "Linux x86_64 · glibc 2.35+ · Terminal edition"
      }
    },
    "footer": "PHP အတွက် Local Environment",
    "source": "Source Code နှင့် ဖြန့်ဝေမှုများ",
    "portHeaders": [
      "Tool / endpoint",
      "မူလ port",
      "အသုံးပြုပုံ"
    ],
    "portRows": [
      [
        "Website",
        "8800",
        "HTTP ပရောဂျက်များ"
      ],
      [
        "Site HTTPS",
        "8443",
        "HTTPS လုံခြုံရေးပါသော .test site"
      ],
      [
        "phpMyAdmin",
        "8801",
        "Browser မှ Database စီမံရန်"
      ],
      [
        "MySQL",
        "13306",
        "သင်၏ App မှ Database ချိတ်ဆက်ရန်"
      ],
      [
        "PHP worker",
        "19000",
        "Nginx → PHP ချိတ်ဆက်မှု"
      ],
      [
        "Mail inbox",
        "8802",
        "Mailpit browser inbox"
      ],
      [
        "Mail SMTP",
        "11025",
        "သင်၏ App မှပေးပို့သော အီးမေးလ်များ စမ်းသပ်ရန်"
      ]
    ],
    "description": "ပရောဂျက်များတည်ဆောက်ခြင်း၊ လိုအပ်သလိုပြင်ဆင်ခြင်း၊ Database နှင့် Mail စီမံခြင်း၊ အခက်အခဲပြဿနာ ဖြေရှင်းခြင်းတို့အပါအဝင် Fairway XMP အသုံးပြုနည်းများ။",
    "contents": "လမ်းညွှန်များ",
    "overview": "အသုံးပြုနည်း လမ်းညွှန်",
    "intro": "Fairway XMP အား အသုံးပြုခြင်းနှင့် လိုအပ်သလို ပြင်ဆင်ခြင်း လမ်းညွှန်များ။",
    "scopeNote": "Windows နှင့် macOS အတွက်သာ ဖြစ်သည်။ Linux command များကို ဖွင့်ကြည့်နိုင်သော မှတ်စုအဖြစ် ထည့်ပေးထားသည်။ နမူနာများတွင် မူလ Port များကို သုံး၍ ပြထားသည်။",
    "popular": "လိုအပ်လေ့ရှိသော လမ်းညွှန်များ",
    "stuck": "ပြဿနာအဖြေရှာနည်း",
    "quickTasks": [
      "sites",
      "mysql",
      "php",
      "mailpit",
      "backups",
      "node"
    ],
    "quickFixes": [
      "site-not-opening",
      "command-not-found",
      "ports",
      "logs"
    ],
    "check": "ရရှိမည့်ရလဒ်",
    "troubleshooting": "ပြဿနာအဖြေရှာနည်း",
    "related": "ဆက်စပ် လမ်းညွှန်များ",
    "screenshotLabel": "Screenshot",
    "screenshot": "နမူနာပြသခြင်းသာ ဖြစ်ပါသည်။ သင်လက်ရှိအသုံးပြုနေသော အခြေအနေနှင့် ကွဲပြားမှု ရှိနိုင်သည်။",
    "linuxTitle": "Linux အတွက်",
    "ports": "Connection ပိုင်း အချက်အလက်များ နှင့် ကြိုတင်သတ်မှတ်ထားသည့် မူလ Port များ",
    "portNote": "ကြိုတင်သတ်မှတ်ထားသည့် မူလတန်ဖိုးများကို ဖော်ပြထားခြင်း ဖြစ်သည်။ ပြင်ဆင်သိမ်းဆည်းထားသည့် လက်ရှိတန်ဖိုးများကို Environment and Service Settings တွင် ကြည့်နိုင်သည်။ သို့မဟုတ် fairway ports Command ကို Run ၍ Linux တွင် ကြည့်နိုင်သည်။",
    "portHelp": "<a href=\"#ports\">Port ညိသည့်အခါ ဖြေရှင်းနည်း</a> · <a href=\"#mysql\">MySQL connection setting များ</a>",
    "nextTitle": "ပြဿနာရှိနေမှုအားသတင်းပို့ရန်",
    "nextText": "အသုံးပြုနေသော Operating System၊ Fairway XMP version နှင့် မျှော်မှန်းသော်ရလဒ်၊ ရရှိသည့် Error တို့ကို ထည့်သွင်း၍ GitHub issue ဖွင့်ပါ။ ပေးပို့သော အချက်အလက်များတွင် Password နှင့် ထိရှလွယ်အချက်အလက်များ မပါစေရန် သတိပြုပါ။",
    "help": "GitHub issue ဖွင့်ရန်",
    "fullDocs": "နည်းပညာ လမ်းညွှန်များ (English)",
    "nav": {
      "docs": "အသုံးပြုနည်းများ",
      "github": "GitHub"
    },
    "scope": "Fairway XMP အသုံးပြုနည်းများ",
    "theme": {
      "label": "Dark mode",
      "toDark": "Dark mode သို့ ပြောင်းရန်",
      "toLight": "Light mode သို့ ပြောင်းရန်"
    }
  }
};
