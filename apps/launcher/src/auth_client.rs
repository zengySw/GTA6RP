use std::{
    net::IpAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use reqwest::{StatusCode, blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use slint::{ComponentHandle, Timer, Weak};

use crate::MainWindow;

#[cfg(windows)]
const SERVICE_NAME: &str = "gta6rp-launcher";
#[cfg(windows)]
const REFRESH_TOKEN_ACCOUNT: &str = "refresh-token";
const DEFAULT_API_BASE: &str = "http://127.0.0.1:8080/api/v1";

#[derive(Serialize)]
struct Credentials<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct RefreshRequest<'a> {
    refresh_token: &'a str,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    user: UserResponse,
}

#[derive(Deserialize)]
struct UserResponse {
    email: String,
}

#[derive(Deserialize)]
struct RegisterResponse {
    email: String,
}

enum Completion {
    SignedIn { email: String },
    Registered { email: String },
    SignedOut,
    StillSignedIn(String),
    RefreshRetry(String),
    Failed(String),
}

const ACCESS_REFRESH_SECONDS: u64 = 14 * 60;
const REFRESH_RETRY_SECONDS: u64 = 30;

pub fn bind(window: &MainWindow) {
    let base_url =
        std::env::var("GTA6RP_API_BASE_URL").unwrap_or_else(|_| DEFAULT_API_BASE.to_owned());
    let access_token = Arc::new(Mutex::new(None::<String>));
    let busy = Arc::new(AtomicBool::new(false));

    let weak_window = window.as_weak();
    let token_state = Arc::clone(&access_token);
    let busy_state = Arc::clone(&busy);
    let api_base = base_url.clone();
    window.on_submit_auth(move |email, password, register| {
        if busy_state.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(window) = weak_window.upgrade() {
            window.set_account_busy(true);
            window.set_account_status("Connecting to the account service…".into());
        }

        let weak_window = weak_window.clone();
        let token_state = Arc::clone(&token_state);
        let busy_state = Arc::clone(&busy_state);
        let api_base = api_base.clone();
        let email = email.to_string();
        let password = password.to_string();
        thread::spawn(move || {
            let completion = if register {
                register_account(&api_base, &email, &password)
                    .map(|user| Completion::Registered { email: user.email })
            } else {
                login(&api_base, &email, &password).and_then(|tokens| {
                    store_refresh_token(&tokens.refresh_token).map_err(|()| {
                        let _ = revoke_session(&api_base, &tokens.refresh_token);
                        "Windows Credential Manager is unavailable; the session was discarded."
                    })?;
                    if let Ok(mut current_token) = token_state.lock() {
                        *current_token = Some(tokens.access_token.clone());
                    }
                    Ok(Completion::SignedIn {
                        email: tokens.user.email,
                    })
                })
            }
            .unwrap_or_else(|message| Completion::Failed(message.to_owned()));

            busy_state.store(false, Ordering::Release);
            report_completion(
                &weak_window,
                &api_base,
                &token_state,
                &busy_state,
                completion,
            );
        });
    });

    let weak_window = window.as_weak();
    let token_state = Arc::clone(&access_token);
    let busy_state = Arc::clone(&busy);
    let api_base = base_url.clone();
    window.on_sign_out(move || {
        if busy_state.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(window) = weak_window.upgrade() {
            window.set_account_busy(true);
            window.set_account_status("Signing out…".into());
        }
        let weak_window = weak_window.clone();
        let token_state = Arc::clone(&token_state);
        let busy_state = Arc::clone(&busy_state);
        let api_base = api_base.clone();
        thread::spawn(move || {
            let completion = if let Some(refresh_token) = load_refresh_token() {
                match revoke_session(&api_base, &refresh_token) {
                    Ok(()) => {
                        delete_refresh_token();
                        if let Ok(mut current_token) = token_state.lock() {
                            *current_token = None;
                        }
                        Completion::SignedOut
                    }
                    Err(message) => Completion::StillSignedIn(message.to_owned()),
                }
            } else {
                if let Ok(mut current_token) = token_state.lock() {
                    *current_token = None;
                }
                Completion::Failed("No saved session was found.".to_owned())
            };
            busy_state.store(false, Ordering::Release);
            report_completion(
                &weak_window,
                &api_base,
                &token_state,
                &busy_state,
                completion,
            );
        });
    });

    restore_session(base_url, window.as_weak(), access_token, busy);
}

fn restore_session(
    api_base: String,
    window: Weak<MainWindow>,
    access_token: Arc<Mutex<Option<String>>>,
    busy: Arc<AtomicBool>,
) {
    busy.store(true, Ordering::Release);
    if let Some(window) = window.upgrade() {
        window.set_account_busy(true);
        window.set_account_status("Checking saved session…".into());
    }
    thread::spawn(move || {
        let completion = match load_refresh_token() {
            Some(refresh_token) => match refresh_session(&api_base, &refresh_token) {
                Ok(tokens) => match store_refresh_token(&tokens.refresh_token) {
                    Ok(()) => {
                        if let Ok(mut current_token) = access_token.lock() {
                            *current_token = Some(tokens.access_token.clone());
                        }
                        Completion::SignedIn {
                            email: tokens.user.email,
                        }
                    }
                    Err(()) => Completion::Failed(
                        "Windows Credential Manager is unavailable; sign in again.".to_owned(),
                    ),
                },
                Err(AuthFailure::Unauthorized) => {
                    delete_refresh_token();
                    Completion::Failed("Saved session expired. Sign in again.".to_owned())
                }
                Err(failure) => Completion::Failed(failure.message().to_owned()),
            },
            None => Completion::Failed("Sign in or create an account.".to_owned()),
        };
        busy.store(false, Ordering::Release);
        report_completion(&window, &api_base, &access_token, &busy, completion);
    });
}

fn report_completion(
    window: &Weak<MainWindow>,
    api_base: &str,
    access_token: &Arc<Mutex<Option<String>>>,
    busy: &Arc<AtomicBool>,
    completion: Completion,
) {
    let window = window.clone();
    let api_base = api_base.to_owned();
    let access_token = Arc::clone(access_token);
    let busy = Arc::clone(busy);
    let _ = slint::invoke_from_event_loop(move || {
        let Some(window) = window.upgrade() else {
            return;
        };
        window.set_account_busy(false);
        match completion {
            Completion::SignedIn { email } => {
                window.set_account_signed_in(true);
                window.set_account_status(format!("Signed in as {email}").into());
                window.set_account_password("".into());
                window.set_account_register_mode(false);
                schedule_refresh(
                    api_base,
                    window.as_weak(),
                    access_token,
                    busy,
                    ACCESS_REFRESH_SECONDS,
                );
            }
            Completion::Registered { email } => {
                window.set_account_signed_in(false);
                window.set_account_status(
                    format!("Account created for {email}. Sign in now.").into(),
                );
                window.set_account_password("".into());
                window.set_account_register_mode(false);
            }
            Completion::SignedOut => {
                window.set_account_signed_in(false);
                window.set_account_status("Signed out.".into());
                if let Ok(mut token) = access_token.lock() {
                    *token = None;
                }
            }
            Completion::StillSignedIn(message) => {
                window.set_account_signed_in(true);
                window.set_account_status(message.into());
            }
            Completion::RefreshRetry(message) => {
                window.set_account_signed_in(true);
                window.set_account_status(message.into());
                schedule_refresh(
                    api_base,
                    window.as_weak(),
                    access_token,
                    busy,
                    REFRESH_RETRY_SECONDS,
                );
            }
            Completion::Failed(message) => {
                window.set_account_signed_in(false);
                window.set_account_status(message.into());
            }
        }
    });
}

fn schedule_refresh(
    api_base: String,
    window: Weak<MainWindow>,
    access_token: Arc<Mutex<Option<String>>>,
    busy: Arc<AtomicBool>,
    delay_seconds: u64,
) {
    Timer::single_shot(Duration::from_secs(delay_seconds), move || {
        if busy.swap(true, Ordering::AcqRel) {
            schedule_refresh(api_base, window, access_token, busy, REFRESH_RETRY_SECONDS);
            return;
        }
        if let Some(window) = window.upgrade() {
            window.set_account_busy(true);
            window.set_account_status("Refreshing the session…".into());
        }

        thread::spawn(move || {
            let completion = if let Some(refresh_token) = load_refresh_token() {
                match refresh_session(&api_base, &refresh_token) {
                    Ok(tokens) => {
                        if store_refresh_token(&tokens.refresh_token).is_ok() {
                            if let Ok(mut current_token) = access_token.lock() {
                                *current_token = Some(tokens.access_token);
                            }
                            Completion::SignedIn {
                                email: tokens.user.email,
                            }
                        } else {
                            let _ = revoke_session(&api_base, &tokens.refresh_token);
                            delete_refresh_token();
                            if let Ok(mut current_token) = access_token.lock() {
                                *current_token = None;
                            }
                            Completion::Failed(
                                "Could not save the refreshed session securely. Sign in again."
                                    .to_owned(),
                            )
                        }
                    }
                    Err(AuthFailure::Unauthorized) => {
                        delete_refresh_token();
                        if let Ok(mut current_token) = access_token.lock() {
                            *current_token = None;
                        }
                        Completion::Failed("Session expired. Sign in again.".to_owned())
                    }
                    Err(failure) => Completion::RefreshRetry(failure.message().to_owned()),
                }
            } else {
                if let Ok(mut current_token) = access_token.lock() {
                    *current_token = None;
                }
                Completion::SignedOut
            };
            busy.store(false, Ordering::Release);
            report_completion(&window, &api_base, &access_token, &busy, completion);
        });
    });
}

fn register_account(
    api_base: &str,
    email: &str,
    password: &str,
) -> Result<RegisterResponse, &'static str> {
    let client = make_client()?;
    post_json(
        &client,
        api_url(api_base, "auth/register")?,
        &Credentials { email, password },
    )
}

fn login(api_base: &str, email: &str, password: &str) -> Result<TokenResponse, &'static str> {
    let client = make_client()?;
    post_json(
        &client,
        api_url(api_base, "auth/login")?,
        &Credentials { email, password },
    )
}

fn refresh_session(api_base: &str, refresh_token: &str) -> Result<TokenResponse, AuthFailure> {
    let client = make_client().map_err(|_| AuthFailure::Service)?;
    post_json(
        &client,
        api_url(api_base, "auth/refresh").map_err(|_| AuthFailure::Service)?,
        &RefreshRequest { refresh_token },
    )
    .map_err(|message| match message {
        "Credentials were rejected." => AuthFailure::Unauthorized,
        "The service could not be reached." => AuthFailure::Network,
        _ => AuthFailure::Service,
    })
}

fn revoke_session(api_base: &str, refresh_token: &str) -> Result<(), &'static str> {
    let client = make_client()?;
    let response = client
        .post(api_url(api_base, "auth/logout")?)
        .json(&RefreshRequest { refresh_token })
        .send()
        .map_err(|_| "The service could not be reached.")?;
    if response.status() == StatusCode::NO_CONTENT {
        Ok(())
    } else if response.status() == StatusCode::UNAUTHORIZED {
        Err("Saved session is no longer valid.")
    } else {
        Err("The service could not complete sign out.")
    }
}

fn post_json<T: DeserializeOwned, B: Serialize>(
    client: &Client,
    url: String,
    body: &B,
) -> Result<T, &'static str> {
    let response = client
        .post(url)
        .json(body)
        .send()
        .map_err(|_| "The service could not be reached.")?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status {
            StatusCode::UNAUTHORIZED => "Credentials were rejected.",
            StatusCode::CONFLICT => "An account with this email already exists.",
            StatusCode::UNPROCESSABLE_ENTITY => {
                "Check the email address and password requirements."
            }
            StatusCode::TOO_MANY_REQUESTS => "Too many requests. Wait before trying again.",
            _ => "The service could not complete the request.",
        });
    }
    response
        .json()
        .map_err(|_| "The service returned an invalid response.")
}

fn make_client() -> Result<Client, &'static str> {
    Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(Policy::none())
        .build()
        .map_err(|_| "The HTTP client could not be initialized.")
}

fn api_url(base_url: &str, suffix: &str) -> Result<String, &'static str> {
    let parsed = reqwest::Url::parse(base_url).map_err(|_| "The API address is invalid.")?;
    let host = parsed.host_str().ok_or("The API address is invalid.")?;
    let loopback = host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if (parsed.scheme() != "https" && !loopback)
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("Use HTTPS for remote API addresses.");
    }
    Ok(format!("{}/{suffix}", base_url.trim_end_matches('/')))
}

#[derive(Clone, Copy)]
enum AuthFailure {
    Unauthorized,
    Network,
    Service,
}

impl AuthFailure {
    fn message(self) -> &'static str {
        match self {
            Self::Unauthorized => "Saved session expired. Sign in again.",
            Self::Network => "Could not reach the account service. The saved session was kept.",
            Self::Service => "The account service returned an unexpected response.",
        }
    }
}

#[cfg(windows)]
fn store_refresh_token(token: &str) -> Result<(), ()> {
    keyring::v1::Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT)
        .map_err(|_| ())?
        .set_password(token)
        .map_err(|_| ())
}

#[cfg(not(windows))]
fn store_refresh_token(_token: &str) -> Result<(), ()> {
    Err(())
}

#[cfg(windows)]
fn load_refresh_token() -> Option<String> {
    keyring::v1::Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT)
        .ok()?
        .get_password()
        .ok()
}

#[cfg(not(windows))]
fn load_refresh_token() -> Option<String> {
    None
}

#[cfg(windows)]
fn delete_refresh_token() {
    if let Ok(entry) = keyring::v1::Entry::new(SERVICE_NAME, REFRESH_TOKEN_ACCOUNT) {
        let _ = entry.delete_credential();
    }
}

#[cfg(not(windows))]
fn delete_refresh_token() {}
