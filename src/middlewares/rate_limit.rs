use actix_governor::{KeyExtractor, SimpleKeyExtractionError};
use std::net::{IpAddr, SocketAddr};
use actix_web::{dev::ServiceRequest, 
    http::header::ContentType, 
    web, HttpResponse, HttpResponseBuilder,
};
use std::str::FromStr;
use actix_governor::governor::{self, clock::{Clock, DefaultClock}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealIpKeyExtractor;

impl KeyExtractor for RealIpKeyExtractor {
    type Key = IpAddr;

    type KeyExtractionError = SimpleKeyExtractionError<&'static str>;

    fn extract(&self, req: &ServiceRequest) -> Result<Self::Key, Self::KeyExtractionError> {
        // Get the reverse proxy IP that we put in app data
        let reverse_proxy_ip = req
            .app_data::<web::Data<IpAddr>>()
            .map(|ip| ip.get_ref().to_owned())
            .unwrap_or_else(|| IpAddr::from_str("0.0.0.0").unwrap());

        let peer_ip = req.peer_addr().map(|socket| socket.ip());
        let connection_info = req.connection_info();

        match peer_ip {
            // The request is coming from the reverse proxy, we can trust the `Forwarded` or `X-Forwarded-For` headers
            Some(peer) if peer == reverse_proxy_ip => connection_info
                .realip_remote_addr()
                .ok_or_else(|| {
                    SimpleKeyExtractionError::new("Could not extract real IP address from request")
                })
                .and_then(|str| {
                    SocketAddr::from_str(str)
                        .map(|socket| socket.ip())
                        .or_else(|_| IpAddr::from_str(str))
                        .map_err(|_| {
                            SimpleKeyExtractionError::new(
                                "Could not extract real IP address from request",
                            )
                        })
                }),
            // The request is not coming from the reverse proxy, we use peer IP
            _ => connection_info
                .peer_addr()
                .ok_or_else(|| {
                    SimpleKeyExtractionError::new("Could not extract peer IP address from request")
                })
                .and_then(|str| {
                    SocketAddr::from_str(str).map_err(|_| {
                        SimpleKeyExtractionError::new(
                            "Could not extract peer IP address from request",
                        )
                    })
                })
                .map(|socket| socket.ip()),
        }
    }


    fn exceed_rate_limit_response(
        &self,
        negative: &governor::NotUntil<governor::clock::QuantaInstant>,
        mut response: HttpResponseBuilder,
    ) -> HttpResponse {
        let wait_time = negative
            .wait_time_from(DefaultClock::default().now())
            .as_secs();
        response.content_type(ContentType::json())
            .body(
                format!(
                    r#"{{"code":429, "error": "TooManyRequests", "message": "Too Many Requests", "after": {wait_time}}}"#
                )
            )
    }
}