#[macro_use]
extern crate lazy_static;

use frame_metadata::RuntimeMetadataPrefixed;
use jsonrpc::serde_json::{to_string, value::RawValue};
use parity_scale_codec::Decode;
use path_tree::PathTree;
use valor::*;

const NODE_ENDPOINT: &str = "http://10.0.17.52:8080";

lazy_static! {
    static ref PATH: PathTree<Action> = {
        let mut p = PathTree::new();
        p.insert("/meta", Action::Meta);
        p.insert("/:module/:item", Action::Storage);
        p
    };
}

type Result<T> = core::result::Result<T, Error>;

enum Action {
    Meta,
    Storage,
}

#[vlugin]
async fn vln(req: Request) -> Response {
    use Method::*;

    let action = PATH.find(req.url().path());
    if action.is_none() {
        return StatusCode::NotFound.into();
    }
    let (action, params) = action.unwrap();

    match (req.method(), action) {
        (Get, Action::Meta) => get_meta().await,
        (Get, Action::Storage) => get_storage(params[0].1, params[1].1).await,
        _ => Ok(StatusCode::MethodNotAllowed.into()),
    }.unwrap_or_else(Into::into)
}

// GET SCALE encoded metadata of the node
async fn get_meta() -> Result<Response> {
    rpc("state_getMetadata", &[])
        .await
        .and_then(meta_as_bytes)
        .map(Into::into)
}

fn meta_as_bytes(data: Box<RawValue>) -> Result<Vec<u8>> {
    let hex_value = &data.get().trim_matches('"')[2..];
    hex::decode(hex_value).map_err(|_| Error::Decode)
}

async fn get_and_decode_meta() -> Result<RuntimeMetadataPrefixed> {
    let meta = get_meta()
        .await?
        .body_bytes()
        .await
        .map_err(|_| Error::Unknown)
        .and_then(|m| 
            RuntimeMetadataPrefixed::decode(&mut &*m)
                .map_err(|_| Error::Decode)
        );
    meta
}

// Query a storage value of the node
async fn get_storage(_module: &str, _name: &str) -> Result<Response> {
    let _meta = get_and_decode_meta().await?;
    rpc("state_getStorage", &[]) // TODO
        .await
        .map(|val| val.get().into())
}

async fn rpc(method: &str, params: &[Box<RawValue>]) -> Result<Box<RawValue>> {
    surf::post(NODE_ENDPOINT)
        .content_type("application/json")
        .body(
            to_string(&jsonrpc::Request {
                id: 1.into(),
                jsonrpc: Some("2.0"),
                method,
                params,
            })
            .unwrap(),
        )
        .await
        .map_err(|_| Error::NodeConnection)?
        .body_json::<jsonrpc::Response>()
        .await
        .map_err(|_| Error::InvalidJSON)?
        .result()
        .map_err(Error::from)
}

enum Error {
    NodeConnection,
    InvalidJSON,
    Rpc(String),
    Decode,
    Unknown,
}

impl From<Error> for Response {
    fn from(e: Error) -> Self {
        match e {
            Error::NodeConnection => StatusCode::BadGateway.into(),
            Error::Rpc(m) => {
                let mut res = Response::new(StatusCode::InternalServerError);
                res.set_body(m);
                res
            },
            _ => StatusCode::InternalServerError.into()
        }
    }
}

impl From<jsonrpc::Error> for Error {
    fn from(err: jsonrpc::Error) -> Self {
        match err {
            jsonrpc::Error::Rpc(e) => Error::Rpc(e.message),
            _ => Error::Unknown,
        }
    }
}
