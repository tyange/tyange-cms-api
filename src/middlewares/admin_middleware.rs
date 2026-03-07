use poem::{Endpoint, Error, Middleware, Request};
use tyange_cms_api::auth::authorization::{current_user, ensure_admin};

pub struct AdminOnly;

impl<E: Endpoint> Middleware<E> for AdminOnly {
    type Output = AdminOnlyImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        AdminOnlyImpl { ep }
    }
}

pub struct AdminOnlyImpl<E> {
    ep: E,
}

impl<E: Endpoint> Endpoint for AdminOnlyImpl<E> {
    type Output = E::Output;

    async fn call(&self, req: Request) -> Result<Self::Output, Error> {
        let user = current_user(&req)?;
        ensure_admin(user)?;
        self.ep.call(req).await
    }
}
