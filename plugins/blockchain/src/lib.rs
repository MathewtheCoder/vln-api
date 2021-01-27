mod meta_util;

use frame_metadata::{RuntimeMetadata, RuntimeMetadataPrefixed, StorageEntryType};
use jsonrpc::serde_json::{to_string, value::RawValue};
use meta_util::*;
use once_cell::sync::OnceCell;
use parity_scale_codec::Decode;
use path_tree::PathTree;
use std::borrow::Cow;
use valor::*;

const NODE_ENDPOINT: &str = "http://10.0.17.52:8080";

static METADATA: OnceCell<RuntimeMetadata> = OnceCell::new();

enum Cmd {
    Meta,
    Storage,
}

type Result<T> = std::result::Result<T, Error>;

#[vlugin]
async fn blockchain_handler(req: Request) -> Response {
    let routes = {
        let mut p = PathTree::new();
        p.insert("/meta", Cmd::Meta);
        p.insert("/:module/:item", Cmd::Storage);
        p
    };

    let url = req.url();
    let action = routes.find(url.path());
    if action.is_none() {
        return StatusCode::NotFound.into();
    }
    let (action, params) = action.unwrap();

    match (req.method(), action) {
        (Method::Get, Cmd::Meta) => get_meta().await,
        (Method::Get, Cmd::Storage) => {
            #[inline]
            fn query_key<'a>(url: &'a Url, name: &str) -> Option<Cow<'a, str>> {
                url.query_pairs().find(|k| k.0 == name).map(|k| k.1)
            }
            get_storage(
                params[0].1,
                params[1].1,
                query_key(url, "k"),
                query_key(url, "k2"),
            )
            .await
        }
        _ => Ok(StatusCode::MethodNotAllowed.into()),
    }
    .unwrap_or_else(Into::into)
}

/// GET the SCALE encoded metadata of the blockchain node
async fn get_meta() -> Result<Response> {
    rpc("state_getMetadata", &[]).await.map(Into::into)
}

async fn get_decoded_meta() -> Result<&'static RuntimeMetadata> {
    let meta = METADATA.get();
    if meta.is_some() {
        return meta.ok_or(Error::Unknown);
    }

    let meta = get_meta()
        .await?
        .body_bytes()
        .await
        .map_err(|_| Error::Unknown)
        .and_then(|m| {
            RuntimeMetadataPrefixed::decode(&mut &*m)
                .map(|m| m.1)
                .map_err(|e| Error::Decode(e.to_string()))
        })?;
    Ok(METADATA.get_or_init(|| meta))
}

/// Query a storage value of the node
async fn get_storage(
    module: &str,
    name: &str,
    _k1: Option<Cow<'_, str>>,
    _k2: Option<Cow<'_, str>>,
) -> Result<Response> {
    let meta = get_decoded_meta().await?;
    let module_name = to_camel(&module.to_lowercase());
    let entry = meta.entry(&module_name, &to_camel(&name.to_lowercase()));
    if entry.is_none() {
        return Ok(StatusCode::NotFound.into());
    }
    let entry = entry.unwrap();

    let mut key = hash_key(&module_name);
    key.push_str(&hash_key(&entry.name.to_string()));

    let key = format!(
        "\"0x{}\"",
        match entry.ty {
            StorageEntryType::Plain(_) => key,
            StorageEntryType::Map { .. } => todo!(),
            StorageEntryType::DoubleMap { .. } => todo!(),
        }
    );

    rpc("state_getStorage", &[&key]).await.map(|val| val.into())
}

fn hash_key(key: &str) -> String {
    use core::hash::Hasher;
    let mut dest: [u8; 16] = [0; 16];

    let mut h0 = twox_hash::XxHash64::with_seed(0);
    let mut h1 = twox_hash::XxHash64::with_seed(1);
    h0.write(key.as_bytes());
    h1.write(key.as_bytes());
    let r0 = h0.finish();
    let r1 = h1.finish();
    use byteorder::{ByteOrder, LittleEndian};
    LittleEndian::write_u64(&mut dest[0..8], r0);
    LittleEndian::write_u64(&mut dest[8..16], r1);
    hex::encode(dest)
}

async fn rpc(method: &str, params: &[&str]) -> Result<Vec<u8>> {
    surf::post(NODE_ENDPOINT)
        .content_type("application/json")
        .body(
            to_string(&jsonrpc::Request {
                id: 1.into(),
                jsonrpc: Some("2.0"),
                method,
                params: &params
                    .iter()
                    .map(|p| RawValue::from_string(p.to_string()).unwrap())
                    .collect::<Vec<_>>(),
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
        .and_then(|s: String| {
            println!("{}", s);
            hex::decode(&s[2..]).map_err(Error::from)
        })
}

fn to_camel(term: &str) -> String {
    let underscore_count = term.chars().filter(|c| *c == '-').count();
    let mut result = String::with_capacity(term.len() - underscore_count);
    let mut at_new_word = true;

    for c in term.chars().skip_while(|&c| c == '-') {
        if c == '-' {
            at_new_word = true;
        } else if at_new_word {
            result.push(c.to_ascii_uppercase());
            at_new_word = false;
        } else {
            result.push(c);
        }
    }
    result
}

enum Error {
    NodeConnection,
    InvalidJSON,
    Rpc(String),
    Decode(String),
    Unknown,
}

impl From<Error> for Response {
    fn from(e: Error) -> Self {
        match e {
            Error::NodeConnection => StatusCode::BadGateway.into(),
            Error::Rpc(e) | Error::Decode(e) => {
                let mut res = Response::new(StatusCode::InternalServerError);
                res.set_body(e);
                res
            }
            _ => StatusCode::InternalServerError.into(),
        }
    }
}

impl From<hex::FromHexError> for Error {
    fn from(err: hex::FromHexError) -> Self {
        Error::Decode(err.to_string())
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
