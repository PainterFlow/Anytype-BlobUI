fn main() {
    blobui_mcp::server::serve_stdio(blobui_mcp::credential::read, |request, cancel, approve| {
        blobui_mcp::transport::call_with_approval(&request, cancel, &|message, remaining| {
            approve(message, remaining)
        })
    });
}
