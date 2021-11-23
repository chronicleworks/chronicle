use common::commands::{ApiCommand, QueryCommand};
use common::models::{Agent, ProvModel};

use wasm_bindgen::prelude::*;
use yew::format::Json;
use yew::prelude::*;

use yew::services::fetch::{Credentials, FetchOptions, FetchService, FetchTask, Request, Response};

use yew_event_source::{EventSourceService, EventSourceStatus, EventSourceTask};

pub struct App {
    link: ComponentLink<Self>,
    shared: Option<ProvModel>,
    es: EventSourceTask,
    ft: Option<FetchTask>,
}

pub enum Msg {
    /// We got new data from the backend.
    EsReady(Result<ProvModel, anyhow::Error>),
    /// Trigger a check of the event source state.
    EsCheckState,
    Ignore,
    Query,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let task = {
            let callback = link.callback(|Json(data)| Msg::EsReady(data));
            let notification = link.callback(|status| {
                if status == EventSourceStatus::Error {
                    log::error!("event source error");
                }
                Msg::EsCheckState
            });
            let mut task = EventSourceService::new()
                .connect("events", notification)
                .unwrap();
            task.add_event_listener("bui_backend", callback);
            task
        };

        Self {
            link,
            shared: None,
            es: task,
            ft: None,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::EsReady(response) => {
                match response {
                    Ok(data_result) => {
                        self.shared = Some(data_result);
                    }
                    Err(e) => {
                        log::error!("{}", e);
                    }
                };
            }
            Msg::EsCheckState => {
                return true;
            }
            Msg::Ignore => {
                return false;
            }
            Msg::Query => {
                self.ft = self.send_message(&ApiCommand::Query(QueryCommand {
                    namespace: "default".to_string(),
                }));
            }
        }
        true
    }

    fn change(&mut self, _: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <div>
                { self.view_ready_state() }
                { self.view_shared() }
                { self.view_input() }
            </div>
        }
    }
}

impl App {
    fn view_ready_state(&self) -> Html {
        html! {
            <p class="field">{ format!("Connection: {:?}", self.es.ready_state()) }</p>
        }
    }

    fn view_shared(&self) -> Html {
        if let Some(ref value) = self.shared {
            html! {
                <p class="field">{ format!("{:?}", value) }</p>
            }
        } else {
            html! {
                <p class="field">{ "Data hasn't fetched yet." }</p>
            }
        }
    }

    fn view_input(&self) -> Html {
        html! {
            <button class="button is-link" onclick=self.link.callback(|_| Msg::Query)>{"Query"}</button>
        }
    }

    fn send_message(&mut self, msg: &ApiCommand) -> Option<yew::services::fetch::FetchTask> {
        let post_request = Request::post("callback")
            .header("Content-Type", "application/json;charset=UTF-8")
            .body(Json(msg))
            .expect("Failed to build request.");
        let callback = self
            .link
            .callback(move |resp: Response<Result<String, _>>| {
                match resp.body() {
                    &Ok(ref _s) => {}
                    &Err(ref e) => {
                        log::error!("Error when sending message: {:?}", e);
                    }
                }
                Msg::Ignore
            });
        let mut options = FetchOptions::default();
        options.credentials = Some(Credentials::SameOrigin);
        match FetchService::fetch_with_options(post_request, options, callback) {
            Ok(task) => Some(task),
            Err(err) => {
                log::error!("sending message failed : {}", err);
                None
            }
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<App>();
}
