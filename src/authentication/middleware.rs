use actix_web::dev::{forward_ready, Service, ServiceRequest, Transform, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::{FromRequest, Error, HttpMessage};
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};

use crate::routes::helpers::ApiError;
use crate::session_state::TypedSession;
use crate::utils::see_other;

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

        match get_current_user_id(session) {
            Ok(current_user_id) => req.extensions_mut().insert(current_user_id),
            Err(err) => return Box::pin(async { Err(login_redirect(err).into()) }),
        };

        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;

            Ok(res)
        })
    }
}

fn get_current_user_id(session: Result<TypedSession, Error>) -> Result<CurrentUserId, ApiError> {
    let user_id = session
        .map_err(|_| ApiError::UnexpectedError(anyhow::anyhow!("Failed to get session")))?
        .get_user_id();

    let user_id = user_id
        .map_err(|_| ApiError::UnexpectedError(anyhow::anyhow!("User is not authorized")))?
        .ok_or_else(|| ApiError::UnexpectedError(anyhow::anyhow!("User is not authorized")))?;

    Ok(CurrentUserId(user_id))
}

fn login_redirect(e: ApiError) -> InternalError<ApiError> { 
    let response = see_other("/login");

    InternalError::from_response(e, response)
}
