use egui::{RichText, Ui};

struct Link {
    name: &'static str,
    url: &'static str,
    note: &'static str,
}

struct Section {
    title: &'static str,
    intro: &'static str,
    links: &'static [Link],
}

const SECTIONS: &[Section] = &[
    Section {
        title: "DAT files",
        intro: "This app parses Logiqx/XML DATs from these cataloging projects — import one from Platforms → Import DAT...",
        links: &[
            Link { name: "No-Intro", url: "https://datomatic.no-intro.org/", note: "Cartridge-based systems (NES, SNES, Game Boy, Genesis...)" },
            Link { name: "Redump", url: "http://redump.org/", note: "Optical disc systems (PlayStation, Saturn, Dreamcast...)" },
            Link { name: "TOSEC", url: "https://www.tosecdev.org/", note: "Broad cross-platform cataloging project" },
        ],
    },
    Section {
        title: "Frontends & distributions",
        intro: "Where this app's Export (Phase 11) and SD/USB imaging (Phase 19) modules send your finished collection.",
        links: &[
            Link { name: "Batocera.linux", url: "https://batocera.org/", note: "Official site and downloadable SD/USB images" },
            Link { name: "Recalbox", url: "https://www.recalbox.com/", note: "Official site and downloadable SD/USB images" },
            Link { name: "Lakka", url: "https://www.lakka.tv/", note: "Official site and downloadable SD/USB images" },
            Link { name: "EmulationStation-DE", url: "https://es-de.org/", note: "Standalone frontend and its gamelist/theme documentation" },
        ],
    },
    Section {
        title: "RetroArch, cores & shaders",
        intro: "Needed by the Shader (Phase 16) and Core Advisor (Phase 17) modules.",
        links: &[
            Link { name: "RetroArch", url: "https://www.retroarch.com/", note: "Official downloads for the RetroArch frontend and libretro cores" },
            Link { name: "libretro/slang-shaders", url: "https://github.com/libretro/slang-shaders", note: "Community shader preset library (each shader has its own license — check before redistributing)" },
        ],
    },
    Section {
        title: "RetroAchievements",
        intro: "Create a free account here, then add your username/API key in Settings to use the RetroAchievements module (Phase 20).",
        links: &[Link { name: "retroachievements.org", url: "https://retroachievements.org/", note: "Account creation and API key (Settings → your profile)" }],
    },
    Section {
        title: "Third-party conversion tools",
        intro: "Not bundled with this app — see docs/COMPILATION.md for setup. Needed by convert to-rvz/from-rvz and to-cso/from-cso.",
        links: &[
            Link { name: "Dolphin Emulator", url: "https://dolphin-emu.org/", note: "Ships DolphinTool, used for RVZ conversion" },
            Link { name: "maxcso", url: "https://github.com/unknownbrackets/maxcso", note: "Used for CSO conversion" },
        ],
    },
];

pub fn show(ui: &mut Ui) {
    egui::ScrollArea::vertical().id_source("download_scroll").auto_shrink([false, false]).show(ui, |ui| {
    ui.heading("Download");
    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "Official/community sites for everything this app works with — DATs, frontends, \
             RetroArch cores/shaders, and the third-party conversion tools. Every link below \
             points at the project's own site or GitHub repository, nothing mirrored here.",
        )
        .weak(),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "This list intentionally does not include ROM download sites. This app manages \
             ROM files you already legally own or have dumped yourself — it has no opinion on \
             where you get them, but won't point you at copyright-infringing sources.",
        )
        .strong()
        .color(egui::Color32::from_rgb(214, 130, 40)),
    );
    ui.add_space(16.0);

    for section in SECTIONS {
        ui.separator();
        ui.add_space(10.0);
        ui.label(RichText::new(section.title).strong().size(16.0));
        ui.add_space(2.0);
        ui.label(RichText::new(section.intro).weak().small());
        ui.add_space(6.0);
        egui::Grid::new(section.title).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            for link in section.links {
                ui.hyperlink_to(link.name, link.url);
                ui.label(RichText::new(link.note).weak().small());
                ui.end_row();
            }
        });
        ui.add_space(6.0);
    }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_link_uses_https_or_http() {
        for section in SECTIONS {
            for link in section.links {
                assert!(link.url.starts_with("https://") || link.url.starts_with("http://"), "{} has a suspicious URL: {}", link.name, link.url);
            }
        }
    }

    #[test]
    fn no_link_points_at_a_rom_hosting_domain() {
        let banned_substrings = ["rom", "iso", "torrent", "warez"];
        for section in SECTIONS {
            for link in section.links {
                let lower = link.url.to_lowercase();
                for banned in banned_substrings {
                    assert!(!lower.contains(banned), "{} looks like it might be a ROM-hosting link: {}", link.name, link.url);
                }
            }
        }
    }

    #[test]
    fn every_section_has_at_least_one_link() {
        for section in SECTIONS {
            assert!(!section.links.is_empty(), "section '{}' has no links", section.title);
        }
    }
}
