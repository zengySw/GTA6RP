use slint::{ModelRc, SharedString, VecModel};

use crate::{MainWindow, NewsCard, ServerCard};

pub fn load(window: &MainWindow) {
    let servers = vec![
        ServerCard {
            name: "Vespucci Stories".into(),
            detail: "North America · English · Roleplay".into(),
            badge: "SAMPLE · ONLINE · 124 / 256".into(),
        },
        ServerCard {
            name: "Pacific Coast RP".into(),
            detail: "Europe · English · Serious roleplay".into(),
            badge: "SAMPLE · ONLINE · 86 / 200".into(),
        },
        ServerCard {
            name: "Los Santos Community".into(),
            detail: "Europe · Ukrainian / English · Casual".into(),
            badge: "SAMPLE · MAINTENANCE".into(),
        },
    ];
    window.set_servers(ModelRc::new(VecModel::from(servers)));

    let news = vec![
        NewsCard {
            title: "Welcome to the launcher preview".into(),
            summary: "This sample article demonstrates the news layout. It is not fetched from a backend.".into(),
            date: "SAMPLE · NOV 21".into(),
        },
        NewsCard {
            title: "What is coming next".into(),
            summary: "The next milestones add fixture installation, server catalog contracts, and backend connectivity.".into(),
            date: "ROADMAP".into(),
        },
    ];
    window.set_news(ModelRc::new(VecModel::from(news)));

    window.set_notice(SharedString::from(
        "Preview mode: server and news catalogs are sample data.",
    ));
}
