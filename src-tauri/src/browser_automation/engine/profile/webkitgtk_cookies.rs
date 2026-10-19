//! WebKitGTK cleanup for the global agent profile, on the GTK main thread
//! inside `with_webview`.

use webkit2gtk::{CookieManagerExt, WebView, WebViewExt, WebsiteDataManagerExt};

use super::is_loopback_domain;

/// Hosts whose cookies the cleanup asks the cookie manager for.
const LOOPBACK_URIS: [&str; 3] = ["http://localhost/", "http://127.0.0.1/", "http://[::1]/"];

/// Delete the persisted cookies loopback hosts left in the tab's profile.
/// Session cookies are not persisted by the engine, so skipping them spares
/// the cookies the tab's own first page is setting right now. Deletion
/// finishes asynchronously on the main loop; false means it was not started.
pub fn clear_loopback_cookies(webview: &WebView) -> bool {
    let Some(manager) = webview
        .website_data_manager()
        .and_then(|data| data.cookie_manager())
    else {
        return false;
    };
    for uri in LOOPBACK_URIS {
        let deleter = manager.clone();
        manager.cookies(uri, None::<&webkit2gtk::gio::Cancellable>, move |found| {
            let Ok(cookies) = found else {
                return;
            };
            for mut cookie in cookies {
                let loopback = cookie.domain().is_some_and(|d| is_loopback_domain(&d));
                if loopback && cookie.expires().is_some() {
                    deleter.delete_cookie(
                        &mut cookie,
                        None::<&webkit2gtk::gio::Cancellable>,
                        |_| {},
                    );
                }
            }
        });
    }
    true
}
