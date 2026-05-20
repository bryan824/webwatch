use snafu::Snafu;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("read config {path}: {source}"))]
    ReadConfig {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("parse config {path}: {source}"))]
    ParseConfig {
        path: String,
        source: toml::de::Error,
    },

    #[snafu(display("config must define at least one target"))]
    EmptyTargets,

    #[snafu(display("target {target_id} must define at least one condition"))]
    EmptyConditions { target_id: String },

    #[snafu(display("parse target URL for target {target_id}: {source}"))]
    ParseTargetUrl {
        target_id: String,
        source: url::ParseError,
    },

    #[snafu(display("condition {condition_id} requires {field}"))]
    MissingConditionField {
        condition_id: String,
        field: &'static str,
    },

    #[snafu(display("invalid CSS selector {selector}: {message}"))]
    InvalidSelector { selector: String, message: String },

    #[snafu(display("browser rendering required: {reason}"))]
    BrowserRequired { reason: String },

    #[snafu(display("browser CDP endpoint not configured"))]
    MissingBrowserCdpUrl,

    #[snafu(display("connect to browser CDP endpoint {url}: {message}"))]
    BrowserConnect { url: String, message: String },

    #[snafu(display("send browser CDP command {method}: {message}"))]
    BrowserSend { method: String, message: String },

    #[snafu(display("read browser CDP response for {method}: {message}"))]
    BrowserRead { method: String, message: String },

    #[snafu(display("browser CDP command {method} failed: {message}"))]
    BrowserProtocol { method: String, message: String },

    #[snafu(display("browser CDP command {method} did not return {field}"))]
    BrowserResponseMissing { method: String, field: String },

    #[snafu(display("parse server bind address {addr}: {source}"))]
    ParseBindAddr {
        addr: String,
        source: std::net::AddrParseError,
    },

    #[snafu(display("bind status API listener {addr}: {source}"))]
    BindListener {
        addr: std::net::SocketAddr,
        source: std::io::Error,
    },

    #[snafu(display("serve status API: {source}"))]
    Serve { source: std::io::Error },

    #[snafu(display("build HTTP client: {source}"))]
    BuildHttpClient { source: reqwest::Error },

    #[snafu(display("database error: {message}"))]
    Database { message: String },

    #[snafu(display("persistence task failed: {message}"))]
    PersistenceTask { message: String },

    #[snafu(display("serialize database JSON: {source}"))]
    SerializeState { source: serde_json::Error },

    #[snafu(display("parse database JSON: {source}"))]
    ParseState { source: serde_json::Error },

    #[snafu(display("request {url}: {source}"))]
    Request { url: String, source: reqwest::Error },

    #[snafu(display("{url} returned HTTP {status}"))]
    HttpStatus {
        url: String,
        status: reqwest::StatusCode,
    },

    #[snafu(display("discord webhook not configured"))]
    MissingDiscordWebhook,

    #[snafu(display("discord webhook returned HTTP {status}: {body}"))]
    DiscordStatus {
        status: reqwest::StatusCode,
        body: String,
    },
}
