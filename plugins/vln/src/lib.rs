use valor::*;

#[vlugin]
async fn vln(_req: Request) -> Response {
    "Hello from plugin".into()
}
