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
