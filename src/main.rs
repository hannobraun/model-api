use rocket::{
    error, get,
    http::{
        hyper::header::{AUTHORIZATION, CACHE_CONTROL},
        Header, Status,
    },
    request::{self, FromRequest},
    response::Responder as RocketResponder,
    routes, Request, Responder,
};
use tempfile::tempdir;
use thiserror::Error;
use tokio::{fs::File, io, process::Command};

#[rocket::launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![spacer])
}

#[get("/models/spacer.3mf?rev=1&<outer>&<inner>&<height>")]
async fn spacer(
    _authorized: Authorized,
    outer: f64,
    inner: f64,
    height: f64,
) -> Result<Model, Error> {
    let tmp = tempdir()?;
    let path = tmp.path().join("model.3mf");
    let path_str = path.as_os_str().to_str().ok_or(TempDirNotValidUtf8Error)?;

    let output = Command::new("openscad")
        .arg(format!("-Douter={}", outer))
        .arg(format!("-Dinner={}", inner))
        .arg(format!("-Dheight={}", height))
        .args(["-o", path_str])
        .arg("api.scad")
        .current_dir("models/spacer")
        .output()
        .await?;

    // This can use `ExitStatus::exit_ok`, once that is stabilized.
    if !output.status.success() {
        return Err(Error::OpenScad(OpenScadError {
            stdout: output.stdout,
            stderr: output.stderr,
        }));
    }

    let file = File::open(path).await?;
    Ok(file.into())
}

// When I still intended to use this API in production, I wanted users to access
// it through a proxy, mostly because that allowed me to cache 3MF files using
// my website's CDN.
//
// The following code is a relatively low-effort way to prevent access to the
// main API endpoint. Making the following key public provides an easy way for
// anyone to circumvent the CDN and run a DoS attack on the service directly.
// Given that generating the 3MF file is quite expensive, this would even be
// very easy to do with few resources.
//
// Preventing this would have required me to build more infrastructure, to load
// the key from (for example) and environment variable. I didn't want to have to
// do that, just to open source this proof of concept. Since I'm not using the
// service for anything critical any more, I decided this is an acceptable
// trade-off.
const AUTH_HEADER: &str = "Basic Wa81oPIR1PtMIJ3cFgkvyCXXeHESx4CV";

struct Authorized;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Authorized {
    type Error = ();

    async fn from_request(
        request: &'r Request<'_>,
    ) -> request::Outcome<Self, Self::Error> {
        let failure = request::Outcome::Failure((Status::Forbidden, ()));
        let auth_header = request.headers().get_one(AUTHORIZATION.as_str());

        let auth_header = match auth_header {
            Some(header) => header,
            None => return failure,
        };

        if auth_header == AUTH_HEADER {
            return request::Outcome::Success(Self);
        }

        failure
    }
}

#[derive(Responder)]
struct Model {
    inner: File,
    cache_control: Header<'static>,
}

impl From<File> for Model {
    fn from(inner: File) -> Self {
        Self {
            inner,
            cache_control: Header::new(
                CACHE_CONTROL.as_str(),
                "public, max-age=31536000, immutable",
            ),
        }
    }
}

// TASK: Add route that returns images of model.

#[derive(Debug, Error, Responder)]
enum Error {
    #[error("I/O error")]
    Io(#[from] io::Error),

    #[error("Temporary directory path is not valid UTF-8")]
    TempDirNotValidUtf8(#[from] TempDirNotValidUtf8Error),

    #[error("Error calling OpenSCAD")]
    OpenScad(OpenScadError),
}

#[derive(Debug, Error)]
#[error("Temporary directory path is not valid UTF-8")]
struct TempDirNotValidUtf8Error;

impl<'r> RocketResponder<'r, 'static> for TempDirNotValidUtf8Error {
    fn respond_to(
        self,
        _: &'r rocket::Request,
    ) -> rocket::response::Result<'static> {
        error!("Temporary directory path is not valid UTF-8",);
        Err(Status::InternalServerError)
    }
}

#[derive(Debug)]
struct OpenScadError {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl<'r> RocketResponder<'r, 'static> for OpenScadError {
    fn respond_to(
        self,
        _: &'r rocket::Request,
    ) -> rocket::response::Result<'static> {
        let stdout = String::from_utf8(self.stdout)
            .unwrap_or_else(|err| format!("Error decoding stdout: {}", err));
        let stderr = String::from_utf8(self.stderr)
            .unwrap_or_else(|err| format!("Error decoding stderr: {}", err));

        error!(
            "Error calling OpenSCAD.\nstdout:\n{}\nstderr:\n{}",
            stdout, stderr
        );

        Err(Status::InternalServerError)
    }
}

#[cfg(test)]
mod tests {
    use rocket::{
        http::{hyper::header::AUTHORIZATION, Header, Status},
        local::blocking::Client,
    };

    #[test]
    fn auth() -> Result<(), rocket::Error> {
        let rocket = super::rocket();
        let client = Client::tracked(rocket)?;

        let path = "/models/spacer.3mf?rev=1&outer=30.0&inner=12.0&height=10.0";

        let response = client.get(path).dispatch();
        assert_eq!(response.status(), Status::Forbidden);

        let response = client
            .get(path)
            .header(Header::new(AUTHORIZATION.as_str(), super::AUTH_HEADER))
            .dispatch();
        assert_eq!(response.status(), Status::Ok);

        Ok(())
    }
}
