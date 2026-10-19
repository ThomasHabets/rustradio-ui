use std::cell::{Cell, OnceCell};

use rustradio::iq_stream::IqSample;

use rustradio_ui::mainthread::spectrum_sink::{WaterfallSink, WaterfallSinkOptions};
use rustradio_ui::mainthread::{get_button, get_element, get_input, send_message};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{AppMessage, Connection, MainToWorker, WorkerToMain};

thread_local! {
    static READY: Cell<bool> = const { Cell::new(false) };
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static FETCHED: Cell<bool> = const { Cell::new(false) };
    static LOADING: Cell<bool> = const { Cell::new(false) };
    static WATERFALL: OnceCell<WaterfallSink> = const { OnceCell::new() };
}

fn status(text: &str) -> Result<(), JsValue> {
    get_element("status")?.set_text_content(Some(text));
    Ok(())
}

// Use the HTTP representation of ListStreams, which is available to browsers
// without a gRPC transport. Keep a stalled server from locking the controls.
#[wasm_bindgen(inline_js = "
    export async function list_streams(url) {
        let response;
        try {
            response = await fetch(url, {signal: AbortSignal.timeout(10000), cache: 'no-store'});
        } catch (error) {
            console.warn('Stream discovery failed', error);
            if (error.name === 'TimeoutError') {
                throw 'Server took too long to respond. Check the address and try again.';
            }
            throw 'Could not reach the server. Check the host, port and TLS setting, then try again.';
        }
        if (!response.ok) {
            throw `Could not fetch streams (HTTP ${response.status}). Check the server and try again.`;
        }
        let streams;
        try {
            streams = await response.json();
        } catch (error) {
            console.warn('Invalid stream list', error);
            throw 'Server did not return a stream list. Check that this is a RustRadio server.';
        }
        if (!Array.isArray(streams) || streams.some(stream =>
            !stream || typeof stream.name !== 'string' || !stream.name ||
            typeof stream.messages !== 'boolean' || typeof stream.ended !== 'boolean')) {
            throw 'Server returned an invalid stream list. Check that this is a RustRadio server.';
        }
        return streams;
    }
")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn list_streams(url: &str) -> Result<web_sys::js_sys::Array, JsValue>;

    type CatalogStream;
    #[wasm_bindgen(method, getter)]
    fn name(this: &CatalogStream) -> String;
    #[wasm_bindgen(method, getter)]
    fn messages(this: &CatalogStream) -> bool;
    #[wasm_bindgen(method, getter)]
    fn ended(this: &CatalogStream) -> bool;
    #[wasm_bindgen(method, getter)]
    fn encoding(this: &CatalogStream) -> Option<CatalogEncoding>;

    type CatalogEncoding;
    #[wasm_bindgen(method, getter)]
    fn component_type(this: &CatalogEncoding) -> i32;
    #[wasm_bindgen(method, getter)]
    fn byte_order(this: &CatalogEncoding) -> i32;
    #[wasm_bindgen(method, getter)]
    fn layout(this: &CatalogEncoding) -> i32;
}

fn source_picker() -> Result<web_sys::HtmlSelectElement, JsValue> {
    Ok(get_element("source")?.dyn_into()?)
}

fn selected_source() -> Result<String, JsValue> {
    let picker = source_picker()?;
    if picker.value().is_empty()
        || !picker
            .query_selector("option:checked")?
            .is_some_and(|option| !option.has_attribute("disabled"))
    {
        return Err(JsValue::from_str(
            "Select an available complex float32 stream.",
        ));
    }
    Ok(picker.value())
}

fn update_controls() -> Result<(), JsValue> {
    let busy = ACTIVE.get() || LOADING.get() || !READY.get();
    get_button("connect")?.set_disabled(busy || selected_source().is_err());
    get_button("disconnect")?.set_disabled(!ACTIVE.get());
    get_button("refresh")?.set_disabled(busy);
    get_button("refresh")?.set_text_content(Some(if LOADING.get() {
        "Fetching streams…"
    } else {
        "Fetch streams →"
    }));
    get_element("stream-step")?
        .set_attribute("data-ready", if FETCHED.get() { "true" } else { "false" })?;
    let stage = if ACTIVE.get() {
        "active"
    } else if LOADING.get() {
        "fetching"
    } else if FETCHED.get() {
        "stream"
    } else {
        "server"
    };
    get_element("connection-steps")?.set_attribute("data-stage", stage)?;
    for name in ["host", "port", "tls"] {
        get_input(name)?.set_disabled(busy);
    }
    source_picker()?.set_disabled(busy || !FETCHED.get());
    get_element("loss-policy")?
        .dyn_into::<web_sys::HtmlSelectElement>()?
        .set_disabled(busy || selected_source().is_err());
    Ok(())
}

fn controls(active: bool) -> Result<(), JsValue> {
    ACTIVE.set(active);
    update_controls()
}

fn show_streams(streams: web_sys::js_sys::Array, previous: &str) -> Result<(), JsValue> {
    let picker = source_picker()?;
    picker.set_text_content(None);
    let document = web_sys::window()
        .expect("main thread")
        .document()
        .expect("document");
    let expected = <rustradio::Complex as IqSample>::encoding();
    let mut first = None;
    let mut keep_previous = false;
    for value in streams.iter() {
        let stream: CatalogStream = value.unchecked_into();
        let compatible = !stream.messages()
            && stream.encoding().is_some_and(|encoding| {
                encoding.component_type() == expected.component_type
                    && encoding.byte_order() == expected.byte_order
                    && encoding.layout() == expected.layout
            });
        let enabled = compatible && !stream.ended();
        let name = stream.name();
        let option = document.create_element("option")?;
        option.set_attribute("value", &name)?;
        let label = if stream.ended() {
            format!("{name} (ended)")
        } else if !compatible {
            format!("{name} (not complex float32)")
        } else {
            name.clone()
        };
        option.set_text_content(Some(&label));
        if enabled {
            first.get_or_insert_with(|| name.clone());
            keep_previous |= name == previous;
        } else {
            option.set_attribute("disabled", "")?;
        }
        picker.append_child(&option)?;
    }
    if streams.length() == 0 {
        let option = document.create_element("option")?;
        option.set_text_content(Some("No streams available"));
        option.set_attribute("disabled", "")?;
        picker.append_child(&option)?;
    }
    picker.set_selected_index(-1);
    if keep_previous {
        picker.set_value(previous);
    } else if let Some(first) = first {
        picker.set_value(&first);
    }
    Ok(())
}

fn invalidate_streams() {
    if ACTIVE.get() || LOADING.get() {
        return;
    }
    FETCHED.set(false);
    source_picker()
        .map(|picker| {
            picker.set_inner_html(
                r#"<option value="" disabled selected>Fetch streams first</option>"#,
            )
        })
        .ok();
    let _ = get_element("stream-hint").map(|hint| {
        hint.set_text_content(Some("Fetch streams from your server to unlock this step."))
    });
    let _ = status("Enter your server address, then fetch streams.");
    let _ = update_controls();
}

fn fetch_streams() {
    if !READY.get() || ACTIVE.get() || LOADING.get() {
        return;
    }
    let result = (|| {
        let previous = source_picker()?.value();
        source_picker()?.set_text_content(None);
        FETCHED.set(false);
        let url = server_url()?;
        url.set_protocol(if url.protocol() == "wss:" {
            "https:"
        } else {
            "http:"
        });
        url.set_pathname("/iq/v1/streams");
        LOADING.set(true);
        update_controls()?;
        status("Fetching available streams…")?;
        spawn_local(async move {
            let result = match list_streams(&url.href()).await {
                Ok(streams) => show_streams(streams, &previous).inspect(|_| FETCHED.set(true)),
                Err(e) => Err(e),
            };
            LOADING.set(false);
            let text = match result {
                Ok(()) if selected_source().is_ok() => {
                    "Streams fetched. Choose a source and start streaming.".into()
                }
                Ok(()) => {
                    "No available complex float32 streams. Fetch streams to try again.".into()
                }
                Err(e) => e.as_string().unwrap_or_else(|| {
                    log::warn!("Could not display stream list: {e:?}");
                    "Could not load streams. Try fetching them again.".into()
                }),
            };
            let _ = get_element("stream-hint").map(|hint| {
                hint.set_text_content(Some(if selected_source().is_ok() {
                    "Choose a source and the mode matching your server’s sink."
                } else {
                    "Fetch a list with an available complex float32 source to continue."
                }))
            });
            let _ = status(&text);
            let _ = update_controls();
        });
        Ok::<_, JsValue>(())
    })();
    if let Err(e) = result {
        LOADING.set(false);
        let _ = status(&e.as_string().unwrap_or_else(|| {
            log::warn!("Could not fetch stream list: {e:?}");
            "Could not fetch streams. Check the server address and try again.".into()
        }));
        let _ = update_controls();
    }
}

fn server_url() -> Result<web_sys::Url, JsValue> {
    let host = get_input("host")?.value().trim().to_owned();
    if host.is_empty()
        || host.contains(['/', '?', '#', '@'])
        || host.chars().any(char::is_whitespace)
    {
        return Err(JsValue::from_str(
            "Enter a hostname or IP address without a URL or path.",
        ));
    }
    let port = get_input("port")?
        .value()
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| JsValue::from_str("Port must be between 1 and 65535."))?;
    // Bracket bare IPv6 addresses before constructing the browser URL.
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
    let scheme = if get_input("tls")?.checked() {
        "wss"
    } else {
        "ws"
    };
    web_sys::Url::new(&format!("{scheme}://{host}:{port}/iq/v1/stream"))
}

fn connection() -> Result<Connection, JsValue> {
    let url = server_url()?;
    let source = selected_source()?;
    let allow_gaps = get_element("loss-policy")?
        .dyn_into::<web_sys::HtmlSelectElement>()?
        .value()
        == "gaps";
    Ok(Connection {
        url: url.href(),
        source,
        allow_gaps,
    })
}

fn connect() {
    if !READY.get() || ACTIVE.get() || LOADING.get() || !FETCHED.get() {
        return;
    }
    let result = (|| {
        let settings = connection()?;
        WATERFALL
            .with(|slot| slot.get().expect("mounted waterfall").clear())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        controls(true)?;
        status("Connecting…")?;
        spawn_local(async move {
            if let Err(e) = send_message(MainToWorker::Start(settings)).await {
                let _ = status(&format!("Connection failed: {e}"));
                let _ = controls(false);
            }
        });
        Ok::<_, JsValue>(())
    })();
    if let Err(e) = result {
        let _ = status(&e.as_string().unwrap_or_else(|| format!("{e:?}")));
    }
}

async fn worker_msg(message: WorkerToMain) -> Result<(), JsValue> {
    match message {
        WorkerToMain::Ready(_) => {
            READY.set(true);
            controls(false)?;
            status("Enter your server address, then fetch streams.")?;
        }
        WorkerToMain::ApplicationSpecific(AppMessage::Connected {
            sample_rate,
            source,
        }) => {
            WATERFALL
                .with(|slot| {
                    slot.get()
                        .expect("mounted waterfall")
                        .set_sample_rate(sample_rate)
                })
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            status(&format!("Streaming {source} · {sample_rate} samples/s"))?;
        }
        WorkerToMain::Floats(name, frames) if name == worker::SPECTRUM => {
            WATERFALL
                .with(|slot| slot.get().expect("mounted waterfall").update(&frames))
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
        }
        WorkerToMain::End(message) => {
            status(&message)?;
            controls(false)?;
        }
        _ => {}
    }
    Ok(())
}

/// Reject stale or incompletely packaged HTML before starting the worker.
fn check_html_version() -> Result<(), JsValue> {
    let html_version = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
        .and_then(|html| html.get_attribute("data-git-version"));
    if html_version.as_deref().is_some_and(|version| {
        !version.is_empty() && version != "GIT_VERSION_NOT_SET" && version == env!("GIT_VERSION")
    }) {
        return Ok(());
    }
    let message = format!(
        "Application version mismatch. HTML: {}; WASM: {}. Reload the page or clear cached assets.",
        html_version.as_deref().unwrap_or("missing"),
        env!("GIT_VERSION")
    );
    status(&message)?;
    Err(JsValue::from_str(&message))
}

pub(crate) fn setup() -> Result<(), JsValue> {
    check_html_version()?;
    let waterfall = WaterfallSink::mount_by_id(
        "waterfall",
        WaterfallSinkOptions {
            title: "I/Q waterfall".into(),
            subtitle: "Frequency relative to the stream center".into(),
            ..Default::default()
        },
    )
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
    WATERFALL.with(|slot| {
        let _ = slot.set(waterfall);
    });
    let refresh_handler = Closure::<dyn FnMut()>::new(fetch_streams);
    get_button("refresh")?
        .add_event_listener_with_callback("click", refresh_handler.as_ref().unchecked_ref())?;
    refresh_handler.forget();
    let address_handler = Closure::<dyn FnMut()>::new(invalidate_streams);
    for name in ["host", "port"] {
        get_input(name)?
            .add_event_listener_with_callback("input", address_handler.as_ref().unchecked_ref())?;
    }
    get_input("tls")?
        .add_event_listener_with_callback("change", address_handler.as_ref().unchecked_ref())?;
    address_handler.forget();
    let source_handler = Closure::<dyn FnMut()>::new(|| {
        let _ = update_controls();
    });
    source_picker()?
        .add_event_listener_with_callback("change", source_handler.as_ref().unchecked_ref())?;
    source_handler.forget();
    let connect_handler = Closure::<dyn FnMut()>::new(connect);
    get_button("connect")?
        .add_event_listener_with_callback("click", connect_handler.as_ref().unchecked_ref())?;
    connect_handler.forget();
    let disconnect_handler = Closure::<dyn FnMut()>::new(|| {
        let _ = get_button("disconnect").map(|button| button.set_disabled(true));
        let _ = status("Stopping stream…");
        spawn_local(async {
            if let Err(e) = send_message(MainToWorker::ApplicationSpecific(AppMessage::Stop)).await
            {
                let _ = status(&e.to_string());
                let _ = controls(false);
            }
        });
    });
    get_button("disconnect")?
        .add_event_listener_with_callback("click", disconnect_handler.as_ref().unchecked_ref())?;
    disconnect_handler.forget();
    get_input("tls")?.set_checked(
        web_sys::window()
            .expect("main thread")
            .location()
            .protocol()?
            == "https:",
    );
    rustradio_ui::mainthread::start_worker::<AppMessage, AppMessage, _, _>(worker_msg);
    Ok(())
}

use crate::worker;
