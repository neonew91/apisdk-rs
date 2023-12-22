use std::collections::HashMap;

use log::LevelFilter;
use reqwest::{header::CONTENT_TYPE, Response, ResponseBuilderExt};
use serde::Serialize;
use serde_json::Value;

use crate::{
    ApiError, ApiResult, FormLike, IntoFilter, LogConfig, Logger, MimeType, MockServer,
    RequestBuilder, RequestId, RequestTraceIdMiddleware, Responder, ResponseBody,
};

/// This struct is used to build RequestConfig internally by macros.
#[derive(Debug, Default)]
pub struct RequestConfigurator {
    /// The target of log
    log_target: &'static str,
    /// Indicate whether to log
    log_filter: Option<log::LevelFilter>,
    /// Indicate whether to parse headers from response or not
    require_headers: bool,
}

impl RequestConfigurator {
    /// Create a new instance
    pub fn new(
        log_target: &'static str,
        log_filter: Option<impl IntoFilter>,
        require_headers: bool,
    ) -> Self {
        Self {
            log_target,
            log_filter: log_filter.and_then(|f| f.into_filter()),
            require_headers,
        }
    }

    /// Update config
    pub fn merge(self, log_target: &'static str, require_headers: bool) -> Self {
        RequestConfigurator {
            log_target,
            require_headers,
            ..self
        }
    }

    /// Build Logger
    fn build(self, req: &mut RequestBuilder) -> (Logger, bool) {
        let extensions = req.extensions();

        let log_filter = extensions
            .get::<LogConfig>()
            .map(|config| config.level)
            .or_else(|| self.log_filter)
            .unwrap_or(LevelFilter::Debug);

        let request_id = extensions
            .get::<RequestId>()
            .map(|id| id.request_id.clone())
            .unwrap_or_default();

        (
            Logger::new(self.log_target, log_filter, request_id),
            self.require_headers,
        )
    }
}

/// Send request
/// - req: used to build request
/// - config: control the send process
pub async fn _send(
    mut req: RequestBuilder,
    config: RequestConfigurator,
) -> ApiResult<ResponseBody> {
    req = RequestTraceIdMiddleware::inject_extension(req);

    let (logger, require_headers) = config.build(&mut req);
    if logger.is_enabled() {
        req = req.with_extension(logger.clone());
    }

    send_and_parse(req, logger, require_headers).await
}

/// Send request with JSON payload
/// - req: used to build request
/// - json: request payload
/// - config: control the send process
pub async fn _send_json<I>(
    mut req: RequestBuilder,
    json: &I,
    config: RequestConfigurator,
) -> ApiResult<ResponseBody>
where
    I: Serialize + ?Sized,
{
    req = RequestTraceIdMiddleware::inject_extension(req);

    req = req.json(json);

    let (logger, require_headers) = config.build(&mut req);
    if logger.is_enabled() {
        req = req.with_extension(
            logger
                .clone()
                .with_json(serde_json::to_value(json).unwrap_or_default()),
        );
    }

    send_and_parse(req, logger, require_headers).await
}

/// Send request with form payload
/// - req: used to build request
/// - form: request payload
/// - config: control the send process
pub async fn _send_form<I>(
    mut req: RequestBuilder,
    form: I,
    config: RequestConfigurator,
) -> ApiResult<ResponseBody>
where
    I: FormLike,
{
    req = RequestTraceIdMiddleware::inject_extension(req);

    let is_multipart = form.is_multipart();
    let meta = form.get_meta();

    if is_multipart {
        if let Some(multipart) = form.get_multipart() {
            req = req.multipart(multipart)
        }
    } else if let Some(form) = form.get_form() {
        req = req.form(&form);
    };

    let (logger, require_headers) = config.build(&mut req);
    if logger.is_enabled() {
        let logger = if is_multipart {
            logger.clone().with_multipart(meta)
        } else {
            logger.clone().with_form(meta)
        };
        req = req.with_extension(logger);
    }

    send_and_parse(req, logger, require_headers).await
}

/// Send request with multipart/data payload
/// - req: used to build request
/// - form: request payload
/// - config: control the send process
pub async fn _send_multipart<I>(
    mut req: RequestBuilder,
    form: I,
    config: RequestConfigurator,
) -> ApiResult<ResponseBody>
where
    I: FormLike,
{
    req = RequestTraceIdMiddleware::inject_extension(req);

    let form = form.get_multipart().ok_or(ApiError::MultipartForm)?;
    let meta = form.get_meta();
    req = req.multipart(form);

    let (logger, require_headers) = config.build(&mut req);
    if logger.is_enabled() {
        req = req.with_extension(logger.clone().with_multipart(meta));
    }

    send_and_parse(req, logger, require_headers).await
}

/// Send request, and get raw response
/// - req: used to build request
/// - config: control the send process
pub async fn _send_raw(
    mut req: RequestBuilder,
    config: RequestConfigurator,
) -> ApiResult<Response> {
    req = RequestTraceIdMiddleware::inject_extension(req);

    let (logger, _) = config.build(&mut req);
    if logger.is_enabled() {
        req = req.with_extension(logger.clone());
    }

    send_and_unparse(req, logger).await
}

/// Send request, and return unparsed response
/// - req: the request to send
/// - logger: helper to log messages
async fn send_and_unparse(mut req: RequestBuilder, logger: Logger) -> ApiResult<Response> {
    let extensions = req.extensions();

    // Mock
    if let Some(mock) = extensions.get::<MockServer>().cloned() {
        let req = req.build().map_err(ApiError::BuildRequest)?;
        logger.log_mock_request_and_response(&req, mock.type_name());
        let url = req.url().clone();
        match mock.handle(req).await {
            Ok(body) => {
                logger.log_mock_response_body(&body);
                let (content_type, text) = match body {
                    ResponseBody::Json(json) => (MimeType::Json, json.to_string()),
                    ResponseBody::Xml(xml) => (MimeType::Xml, xml),
                    ResponseBody::Text(text) => (MimeType::Text, text),
                };
                let res = hyper::Response::builder()
                    .url(url)
                    .header(CONTENT_TYPE, content_type.to_string())
                    .body(text)
                    .map_err(|_| {
                        ApiError::Middleware(anyhow::format_err!("Failed to build response"))
                    })?;
                return Ok(Response::from(res));
            }
            Err(e) => {
                logger.log_error(&e);
                return Err(ApiError::Middleware(e));
            }
        }
    }

    let res = req.send().await?;
    Ok(res)
}

/// Send request, and parse response as desired type
/// - req: the request to send
/// - logger: helper to log messages
/// - require_headers: should zip headers into response body
async fn send_and_parse(
    mut req: RequestBuilder,
    logger: Logger,
    require_headers: bool,
) -> ApiResult<ResponseBody> {
    let extensions = req.extensions();

    // Mock
    if let Some(mock) = extensions.get::<MockServer>().cloned() {
        let req = req.build().map_err(ApiError::BuildRequest)?;
        logger.log_mock_request_and_response(&req, mock.type_name());
        match mock.handle(req).await {
            Ok(body) => {
                logger.log_mock_response_body(&body);
                return Ok(body);
            }
            Err(e) => {
                logger.log_error(&e);
                return Err(ApiError::Middleware(e));
            }
        }
    }

    // Send the request
    let res = req.send().await?;

    // Check status code
    let status = res.status();
    let res = if status.is_client_error() || status.is_server_error() {
        let e = if status.is_client_error() {
            ApiError::HttpClientStatus(status.as_u16(), status.to_string())
        } else {
            ApiError::HttpServerStatus(status.as_u16(), status.to_string())
        };
        logger.log_error(&e);
        return Err(e);
    } else {
        res
    };

    // Check content-type, and parse payload
    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(MimeType::from)
        .unwrap_or(MimeType::Text);
    match content_type {
        MimeType::Json => parse_as_json(res, content_type, logger, require_headers).await,
        MimeType::Xml => parse_as_xml(res, content_type, logger).await,
        MimeType::Text => parse_as_text(res, content_type, logger).await,
        _ => Err(ApiError::UnsupportedContentType(content_type)),
    }
}

/// Parse response body to json
async fn parse_as_json(
    res: Response,
    content_type: MimeType,
    logger: Logger,
    require_headers: bool,
) -> ApiResult<ResponseBody> {
    // Extract HTTP headers from response
    let headers = if require_headers {
        let mut headers = HashMap::new();
        for (name, value) in res.headers() {
            if let Ok(value) = value.to_str() {
                headers.insert(name.to_string(), value.to_string());
            }
        }
        Some(headers)
    } else {
        None
    };

    // Decode response
    let mut json = match res.json::<Value>().await {
        Ok(json) => {
            logger.log_response_json(&json);
            json
        }
        Err(e) => {
            let e = ApiError::DecodeResponse(content_type, e.to_string());
            logger.log_error(&e);
            return Err(e);
        }
    };

    // Inject headers as `__headers__` field into payload
    // Extractor could parse the `__headers__` field if required
    if let Some(headers) = headers {
        if let Value::Object(m) = &mut json {
            if let Ok(headers) = serde_json::to_value(headers) {
                m.insert("__headers__".to_string(), headers);
            }
        }
    }

    Ok(ResponseBody::Json(json))
}

/// Parse response body to xml
async fn parse_as_xml(
    res: Response,
    content_type: MimeType,
    logger: Logger,
) -> ApiResult<ResponseBody> {
    // Decode response as text
    let text = match res.text().await {
        Ok(text) => {
            logger.log_response_xml(&text);
            text
        }
        Err(e) => {
            let e = ApiError::DecodeResponse(content_type, e.to_string());
            logger.log_error(&e);
            return Err(e);
        }
    };

    Ok(ResponseBody::Xml(text))
}

/// Parse response body to text
async fn parse_as_text(
    res: Response,
    content_type: MimeType,
    logger: Logger,
) -> ApiResult<ResponseBody> {
    // Decode response
    let text = match res.text().await {
        Ok(text) => {
            logger.log_response_text(&text);
            text
        }
        Err(e) => {
            let e = ApiError::DecodeResponse(content_type, e.to_string());
            logger.log_error(&e);
            return Err(e);
        }
    };

    Ok(ResponseBody::Text(text))
}
