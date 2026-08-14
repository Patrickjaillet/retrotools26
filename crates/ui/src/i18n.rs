/// Minimal internationalization layer: an enum of translatable UI strings,
/// resolved against the active language at render time. Only a
/// representative subset of the UI is wired up (tab names, the dashboard,
/// and the most common actions) — enough to prove the structure actually
/// works end-to-end (switching language in Settings updates the UI
/// immediately) rather than translating every label in the app, which the
/// roadmap doesn't ask for ("structure i18n prête, anglais par défaut").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Fr,
}

impl Language {
    pub fn from_code(code: &str) -> Self {
        match code {
            "fr" => Language::Fr,
            _ => Language::En,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Fr => "fr",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Fr => "Français",
        }
    }

    pub fn all() -> &'static [Language] {
        &[Language::En, Language::Fr]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    TabDashboard,
    TabPlatforms,
    TabGames,
    Tab1g1r,
    TabPlugins,
    TabSettings,
    TabDownload,
    TabAbout,
    DashboardTitle,
    DashboardPlatforms,
    DashboardDatFilesLoaded,
    DashboardGamesTracked,
    DashboardCompletion,
    DashboardNoActivity,
    DashboardImportPrompt,
    ImportDatButton,
    ChooseRomFolderButton,
    StartScanButton,
    PreviewSelectionButton,
    SaveSettingsButton,
    SettingsTitle,
    SettingsAppearance,
    SettingsTheme,
    SettingsLanguage,
    ExpertModeToggle,
}

pub fn t(lang: Language, key: Key) -> &'static str {
    use Key::*;
    use Language::*;
    match (lang, key) {
        (En, TabDashboard) => "Dashboard",
        (Fr, TabDashboard) => "Tableau de bord",

        (En, TabPlatforms) => "Platforms",
        (Fr, TabPlatforms) => "Plateformes",

        (En, TabGames) => "Games",
        (Fr, TabGames) => "Jeux",

        (En, Tab1g1r) => "1G1R",
        (Fr, Tab1g1r) => "1G1R",

        (En, TabPlugins) => "Plugins",
        (Fr, TabPlugins) => "Extensions",

        (En, TabSettings) => "Settings",
        (Fr, TabSettings) => "Paramètres",

        (En, TabDownload) => "Download",
        (Fr, TabDownload) => "Téléchargements",

        (En, TabAbout) => "About",
        (Fr, TabAbout) => "À propos",

        (En, DashboardTitle) => "Dashboard",
        (Fr, DashboardTitle) => "Tableau de bord",

        (En, DashboardPlatforms) => "Platforms",
        (Fr, DashboardPlatforms) => "Plateformes",

        (En, DashboardDatFilesLoaded) => "DAT files loaded",
        (Fr, DashboardDatFilesLoaded) => "Fichiers DAT chargés",

        (En, DashboardGamesTracked) => "Games tracked",
        (Fr, DashboardGamesTracked) => "Jeux suivis",

        (En, DashboardCompletion) => "Completion",
        (Fr, DashboardCompletion) => "Complétion",

        (En, DashboardNoActivity) => "No recent activity yet.",
        (Fr, DashboardNoActivity) => "Aucune activité récente.",

        (En, DashboardImportPrompt) => {
            "Import a DAT file and scan your ROM directories to get started."
        }
        (Fr, DashboardImportPrompt) => {
            "Importez un fichier DAT et scannez vos dossiers de ROMs pour commencer."
        }

        (En, ImportDatButton) => "Import DAT...",
        (Fr, ImportDatButton) => "Importer un DAT...",

        (En, ChooseRomFolderButton) => "Choose ROM folder...",
        (Fr, ChooseRomFolderButton) => "Choisir le dossier ROM...",

        (En, StartScanButton) => "Start scan",
        (Fr, StartScanButton) => "Lancer le scan",

        (En, PreviewSelectionButton) => "Preview 1G1R selection",
        (Fr, PreviewSelectionButton) => "Prévisualiser la sélection 1G1R",

        (En, SaveSettingsButton) => "Save settings",
        (Fr, SaveSettingsButton) => "Enregistrer les paramètres",

        (En, SettingsTitle) => "Settings",
        (Fr, SettingsTitle) => "Paramètres",

        (En, SettingsAppearance) => "Appearance",
        (Fr, SettingsAppearance) => "Apparence",

        (En, SettingsTheme) => "Theme:",
        (Fr, SettingsTheme) => "Thème :",

        (En, SettingsLanguage) => "Language:",
        (Fr, SettingsLanguage) => "Langue :",

        (En, ExpertModeToggle) => "Expert mode (show every option on one page)",
        (Fr, ExpertModeToggle) => "Mode expert (toutes les options sur une page)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_language_codes() {
        assert_eq!(Language::from_code("fr"), Language::Fr);
        assert_eq!(Language::from_code("en"), Language::En);
        assert_eq!(Language::from_code("unknown"), Language::En);
        assert_eq!(Language::Fr.code(), "fr");
        assert_eq!(Language::En.code(), "en");
    }

    #[test]
    fn every_key_resolves_in_every_language() {
        let keys = [
            Key::TabDashboard,
            Key::TabPlatforms,
            Key::TabGames,
            Key::Tab1g1r,
            Key::TabPlugins,
            Key::TabSettings,
            Key::TabDownload,
            Key::TabAbout,
            Key::DashboardTitle,
            Key::DashboardPlatforms,
            Key::DashboardDatFilesLoaded,
            Key::DashboardGamesTracked,
            Key::DashboardCompletion,
            Key::DashboardNoActivity,
            Key::DashboardImportPrompt,
            Key::ImportDatButton,
            Key::ChooseRomFolderButton,
            Key::StartScanButton,
            Key::PreviewSelectionButton,
            Key::SaveSettingsButton,
            Key::SettingsTitle,
            Key::SettingsAppearance,
            Key::SettingsTheme,
            Key::SettingsLanguage,
            Key::ExpertModeToggle,
        ];
        for lang in Language::all() {
            for key in keys {
                assert!(!t(*lang, key).is_empty());
            }
        }
    }

    #[test]
    fn translations_actually_differ_between_languages() {
        assert_ne!(
            t(Language::En, Key::TabDashboard),
            t(Language::Fr, Key::TabDashboard)
        );
    }
}
