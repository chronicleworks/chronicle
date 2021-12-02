use common::commands::{ApiCommand, QueryCommand};
use common::prov::ProvModel;

use wasm_bindgen::prelude::*;

use ybc::TileCtx::Parent;
use ybc::TileSize::Eight;

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

#[derive(Properties, Clone, PartialEq)]
pub struct AgentProps {
    agent: common::prov::Agent,
}

pub struct Agent {
    props: AgentProps,
}

impl Component for Agent {
    type Message = ();
    type Properties = AgentProps;

    fn create(props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Self { props }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props == props
    }

    fn view(&self) -> Html {
        html! {
            <ybc::Tile ctx=Parent vertical=true size=Eight>
                <ybc::Panel classes={classes!{"is-primary"}} heading={
                    html!{
                        <>
                            <yew_feather::user::User class="mr-4"/>
                            {&self.props.agent.name}
                        </>
                    }}>
                </ybc::Panel>
            </ybc::Tile>
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct ActivityProps {
    activity: common::prov::Activity,
}

pub struct Activity {
    props: ActivityProps,
}

impl Component for Activity {
    type Message = ();
    type Properties = ActivityProps;

    fn create(props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Self { props }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props == props
    }

    fn view(&self) -> Html {
        html! {
            <ybc::Tile ctx=Parent vertical=true size=Eight>
                <ybc::Panel classes={classes!{"is-link"}} heading={
                    html!{
                        <>
                            <yew_feather::activity::Activity class="mr-4"/>
                            {&self.props.activity.name}
                        </>
                    }}>
                    <ybc::PanelBlock>
                        {for self.props.activity.started.map(|started| html!{
                            <>
                            <ybc::Subtitle>
                                {"Started"}
                            </ybc::Subtitle>
                            <br/>
                            {started.to_rfc2822()}
                            </>
                        })}
                        {for self.props.activity.ended.map(|started| html!{
                            <>
                            <ybc::Subtitle>
                                {"Ended"}
                            </ybc::Subtitle>
                            <br/>
                            {started.to_rfc2822()}
                            </>
                        })}
                    </ybc::PanelBlock>
                </ybc::Panel>
            </ybc::Tile>
        }
    }
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
            Msg::EsReady(response) => match response {
                Ok(data_result) => {
                    self.shared = Some(data_result);
                    true
                }
                Err(e) => {
                    log::error!("{}", e);
                    false
                }
            },
            Msg::EsCheckState => true,
            Msg::Ignore => false,
            Msg::Query => {
                self.ft = self.send_message(&ApiCommand::Query(QueryCommand {
                    namespace: "default".to_string(),
                }));
                false
            }
        }
    }

    fn change(&mut self, _: Self::Properties) -> ShouldRender {
        false
    }

    fn rendered(&mut self, first_render: bool) {
        if first_render {
            self.ft = self.send_message(&ApiCommand::Query(QueryCommand {
                namespace: "default".to_string(),
            }));
        }
    }

    fn view(&self) -> Html {
        html! {
           <ybc::Container fluid=true>
            <ybc::Title>{"Agents"}</ybc::Title>
                {self.view_agents()}
            <ybc::Title>{"Activities"}</ybc::Title>
                {self.view_activities()}
           </ybc::Container>
        }
    }
}

impl App {
    fn view_agents(&self) -> Html {
        if let Some(ref value) = self.shared {
            let agents = value
                .agents
                .values()
                .map(|agent| html! {<Agent agent={agent.clone()}/>})
                .collect::<Vec<_>>();
            html! {
                <>
                    {for agents}
                </>
            }
        } else {
            html! {}
        }
    }

    fn view_activities(&self) -> Html {
        if let Some(ref value) = self.shared {
            let activities = value
                .activities
                .values()
                .map(|activity| html! {<Activity activity={activity.clone()}/>})
                .collect::<Vec<_>>();
            html! {
                <>
                    {for activities}
                </>
            }
        } else {
            html! {}
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
