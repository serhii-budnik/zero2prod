use actix_web::dev::{forward_ready, Service, ServiceRequest, Transform, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::http::header::LOCATION;
use actix_web::{FromRequest, Error, HttpResponse, HttpMessage};
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};

use crate::routes::helpers::ApiError;
use crate::session_state::TypedSession;

#[derive(Debug, Copy, Clone)]
pub struct CurrentUserId(pub uuid::Uuid);

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct RejectAnonymousUsers;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for RejectAnonymousUsers
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RejectAnonymousUsersMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RejectAnonymousUsersMiddleware { service }))
    }
}

pub struct RejectAnonymousUsersMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RejectAnonymousUsersMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let session = {
            let (http_request, payload) = req.parts_mut();

            Box::pin(async move { 
                TypedSession::from_request(http_request, payload).await
            })
        };

        let session = futures::executor::block_on(session);

        let user_id = match session { 
            Ok(sess) => sess.get_user_id(),
            Err(_) => { 
                return Box::pin(async { 
                    Err(login_redirect(ApiError::UnexpectedError(anyhow::anyhow!("Failed to get session"))).into())
                })
            }
        };

        match user_id {
            Ok(id) => req.extensions_mut().insert(CurrentUserId(id.unwrap())),
            Err(_) => {
                return Box::pin(async move { 
                    Err(login_redirect(ApiError::UnexpectedError(anyhow::anyhow!("User is not authorized"))).into())
                });
            }
        };

        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;

            Ok(res)
        })
    }
}

fn login_redirect(e: ApiError) -> InternalError<ApiError> { 
    let response = HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish();

    InternalError::from_response(e, response)
}
