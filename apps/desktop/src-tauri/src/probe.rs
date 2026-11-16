pub fn parse_engine_response(response: &str) -> Result<(), String> {
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .ok_or("Local port is not serving a Bansayo state response.")?;
    let json: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "Local port is not serving a Bansayo state response.")?;
    if response.starts_with("HTTP/1.1 200")
        && json["ok"] == true
        && json["data"]["engine"]["protocol_version"] == 1
    {
        Ok(())
    } else {
        Err("Port 7700 is in use by an incompatible service. Stop it or run a compatible Bansayo engine.".into())
    }
}
#[cfg(test)]
mod tests {
    use super::parse_engine_response;
    #[test]
    fn only_attaches_to_compatible_engine_state() {
        let response =
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true,\"data\":{\"engine\":{\"protocol_version\":1}}}";
        assert!(parse_engine_response(response).is_ok());
        assert!(parse_engine_response(&response.replace("version\":1", "version\":99")).is_err());
        assert!(parse_engine_response("HTTP/1.1 200 OK\r\n\r\nnot json").is_err());
    }
}
