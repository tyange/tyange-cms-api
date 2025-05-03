use poem::{Endpoint, IntoResponse, Middleware, Response};

pub struct Auth;

impl<E: Endpoint> Middleware<E> for Auth {
    type Output = AuthImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        AuthImpl(ep)
    }
}

pub struct AuthImpl<E>(E);

impl<E: Endpoint> Endpoint for AuthImpl<E> {
    type Output = Response;

    async fn call(&self, req: poem::Request) -> poem::Result<Self::Output> {
        println!("requset: {}", req.uri().path());
        let res = self.0.call(req).await;

        match res {
            Ok(resp) => {
                let resp = resp.into_response();
                println!("response: {}", resp.status());
                Ok(resp)
            }
            Err(err) => {
                println!("error: {err}");
                Err(err)
            }
        }
    }
}
