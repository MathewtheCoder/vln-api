use valor::*;

#[vlugin]
fn handle_request(_req: Request) -> Response {
    "Hello from plugin".into()
}
