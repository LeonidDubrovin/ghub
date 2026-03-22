/// Shared scanning configuration constants
/// These values are used by both the background scanning service and the synchronous scanner

/// Maximum depth for recursive directory scanning
pub const MAX_SCAN_DEPTH: usize = 5;

/// Maximum depth for searching executables within a game folder
pub const MAX_EXE_SEARCH_DEPTH: usize = 4;

/// Maximum number of cover candidates to return
pub const MAX_COVER_CANDIDATES: usize = 15;

/// Maximum depth for searching cover images
pub const MAX_COVER_SEARCH_DEPTH: usize = 3;

/// Maximum depth for searching within subdirectories to find the actual game folder
/// (currently unused, reserved for future use)
pub const _MAX_GAME_FOLDER_SEARCH_DEPTH: usize = 2;

/// Default metadata file names to search for
pub const BASE_METADATA_FILES: &[&str] = &[
    "game.json", "info.json", "metadata.json", "gameinfo.json",
    "game.yaml", "game.yml", "info.yaml", "info.yml", "metadata.yaml", "metadata.yml",
    "game.toml", "info.toml", "metadata.toml",
    "game.xml", "info.xml", "metadata.xml",
    "info.txt", "readme.txt", "README.md", "README.txt", "about.txt", "description.txt", "game_info.txt",
    "manifest.json", "package.json", "config.json", "UnityManifest.json", "ProjectSettings.asset", "DefaultGame.ini", "Game.ini", "config.ini",
];

/// Default exe exclusion patterns (regex strings)
pub const BASE_EXE_EXCLUSIONS: &[&str] = &[
    r"(?i)unins\d*",
    r"(?i)^setup",
    r"(?i)^install",
    r"(?i)vc_redist\.(x64|x86)",
    r"(?i)dxsetup",
    r"(?i)directx",
    r"(?i)dotnet",
    r"(?i)crashreport",
    r"(?i)crash\s*handler",
    r"(?i)launcher$",
    r"(?i)updater$",
    r"(?i)ue4prereq",
    r"(?i)physx",
    r"(?i)steamcmd",
    r"(?i)easyanticheat",
    r"(?i)battleye",
    r"(?i)^notification_helper\.exe$",
    r"(?i)^unitycrashhandler(32|64)\.exe$",
    r"(?i)^python(w)?\.exe$",
    r"(?i)^zsync(make)?\.exe$",
    // Additional utility/installer exclusions based on scan analysis
    r"(?i)^createdump\.exe$",
    r"(?i)^epicwebhelper\.exe$",
    r"(?i)^oalinst\.exe$",
    r"(?i)^bcl\.exe$",
    r"(?i)^index\.exe$",
    r"(?i)^cli-(32|64)\.exe$",
    r"(?i)^run\.exe$",
    r"(?i)^game\.exe$",
    r"(?i)^start\.exe$",
    r"(?i)^launch\.exe$",
    r"(?i)^test\.exe$",
    r"(?i)^demo\.exe$",
    r"(?i)^sample\.exe$",
    r"(?i)^example\.exe$",
    r"(?i)^tutorial\.exe$",
    r"(?i)^template\.exe$",
    r"(?i)^helper\.exe$",
    r"(?i)^tool\.exe$",
    r"(?i)^utility\.exe$",
    r"(?i)^config\.exe$",
    r"(?i)^settings\.exe$",
    r"(?i)^options\.exe$",
    r"(?i)^bootstrap\.exe$",
    r"(?i)^packagedgame\.exe$",
    r"(?i)^windowsnoeditor\.exe$",
    r"(?i)^ue4(editor|game)?\.exe$",
    r"(?i)^ue5(editor|game)?\.exe$",
    r"(?i)^unity(editor|player)?\.exe$",
    r"(?i)^godot\.exe$",
    r"(?i)^gdx\.exe$",
    r"(?i)^xna\.exe$",
    r"(?i)^monogame\.exe$",
    r"(?i)^rpgmaker\.exe$",
    r"(?i)^gamemaker\.exe$",
    r"(?i)^construct\.exe$",
    r"(?i)^clickteam\.exe$",
    r"(?i)^fusion\.exe$",
    r"(?i)^realbasic\.exe$",
    r"(?i)^delphi\.exe$",
    r"(?i)^visualbasic\.exe$",
    r"(?i)^vb\.exe$",
    r"(?i)^dotnet\.exe$",
    r"(?i)^framework\.exe$",
    r"(?i)^microsoft\.exe$",
    r"(?i)^microsoft\.visualbasic\.exe$",
    r"(?i)^microsoft\.net\.exe$",
    r"(?i)^java\.exe$",
    r"(?i)^javaw\.exe$",
    r"(?i)^jre\.exe$",
    r"(?i)^jdk\.exe$",
    r"(?i)^node\.exe$",
    r"(?i)^npm\.exe$",
    r"(?i)^yarn\.exe$",
    r"(?i)^pnpm\.exe$",
    r"(?i)^bun\.exe$",
    r"(?i)^deno\.exe$",
    r"(?i)^go\.exe$",
    r"(?i)^rust\.exe$",
    r"(?i)^cargo\.exe$",
    r"(?i)^gcc\.exe$",
    r"(?i)^g\+\+\.exe$",
    r"(?i)^clang\.exe$",
    r"(?i)^clang\+\+\.exe$",
    r"(?i)^make\.exe$",
    r"(?i)^cmake\.exe$",
    r"(?i)^autotools\.exe$",
    r"(?i)^autoconf\.exe$",
    r"(?i)^automake\.exe$",
    r"(?i)^libtool\.exe$",
    r"(?i)^meson\.exe$",
    r"(?i)^ninja\.exe$",
    r"(?i)^scons\.exe$",
    r"(?i)^ant\.exe$",
    r"(?i)^maven\.exe$",
    r"(?i)^gradle\.exe$",
    r"(?i)^msbuild\.exe$",
    r"(?i)^devenv\.exe$",
    r"(?i)^visualstudio\.exe$",
    r"(?i)^vs\.exe$",
    r"(?i)^xcode\.exe$",
    r"(?i)^xcodebuild\.exe$",
    r"(?i)^xcode-select\.exe$",
    // Additional patterns from scan analysis
    r"(?i)ueprereq",
    r"(?i)prereq",
    r"(?i)crashpad",
    r"(?i)^nw\.exe$",
    r"(?i)oainst\.exe$",
    // Java utilities (JRE/JDK tools that are not games)
    r"(?i)^jaccessinspector\.exe$",
    r"(?i)^javaw?\.exe$",
    r"(?i)^javac\.exe$",
    r"(?i)^keytool\.exe$",
    r"(?i)^jarsigner\.exe$",
    r"(?i)^javap\.exe$",
    r"(?i)^jps\.exe$",
    r"(?i)^jstat\.exe$",
    r"(?i)^jstack\.exe$",
    r"(?i)^jmap\.exe$",
    r"(?i)^jinfo\.exe$",
    r"(?i)^jdb\.exe$",
    r"(?i)^jshell\.exe$",
    // Additional Java-related utilities
    r"(?i)^javaws\.exe$",
    r"(?i)^jabswitch\.exe$",
    r"(?i)^jexec\.exe$",
];

/// Default folder exclusion patterns (regex strings)
pub const BASE_FOLDER_EXCLUSIONS: &[&str] = &[
    r"(?i)^(engine|redist|redistributables)$",
    r"(?i)^(directx|dotnet|vcredist|physx)$",
    r"(?i)^(prereqs?|prerequisites|support)$",
    r"(?i)^(commonredist|installer|install|setup)$",
    r"(?i)^(update|patch(es)?|backup)$",
    r"(?i)^(temp|tmp|cache|logs)$",
    r"(?i)^(saves?|screenshots?|mods?|plugins?)$",
    r"(?i)^binaries$",
    r"(?i)^__pycache__$",
    r"(?i)^\.git$",
    // Additional folder exclusions from scan analysis
    r"(?i)^\.hg$",
    r"(?i)^\.svn$",
    r"(?i)^\.pytest_cache$",
    r"(?i)^\.cache$",
    r"(?i)^\.vscode$",
    r"(?i)^\.idea$",
    r"(?i)^lib$",
    r"(?i)^library$",
    r"(?i)^packages$",
    r"(?i)^pkgcache$",
    r"(?i)^packagecache$",
    r"(?i)^node_modules$",
    r"(?i)^vendor$",
    r"(?i)^thirdparty$",
    r"(?i)^third_party$",
    r"(?i)^deps$",
    r"(?i)^dependencies$",
    r"(?i)^build$",
    r"(?i)^dist$",
    r"(?i)^out$",
    r"(?i)^output$",
    r"(?i)^target$",
    // Runtime/engine support folders that should not be scanned as games
    r"(?i)^jre$",
    r"(?i)^jdk$",
    r"(?i)^runtime$",
    r"(?i)^runtimes$",
    r"(?i)^engine$",
    // Additional common utility folders
    r"(?i)^bin$",
    r"(?i)^win$",
    r"(?i)^windows$",
    r"(?i)^x64$",
    r"(?i)^x86$",
    r"(?i)^__MACOSX$",
    r"(?i)^gmlive$",
    // Language/culture folders (e.g., en-us, fr-fr, de-de, etc.)
    r"(?i)^[a-z]{2}-[a-z]{2}$",
];

/// Default image extensions to search for
pub const BASE_IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "ico", "bmp", "webp", "gif",
];

/// Default cover search paths (subdirectories to search for covers)
pub const BASE_COVER_SEARCH_PATHS: &[&str] = &[
    "images", "image", "img", "art", "assets", "media", "resources",
    "gfx", "graphics", "covers", "cover", "box", "boxart", "screenshots",
    "screenshot", "promo",
];
