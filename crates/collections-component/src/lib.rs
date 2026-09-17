//! External, read-only structural shadow for Vqro terminal topology.

pub mod projection;
pub mod service;

#[cfg(target_arch = "wasm32")]
mod guest {
    wit_bindgen::generate!({
        path: "../../contracts/vqro-extension-service",
        world: "service",
    });

    use self::vqro::extension::host;

    struct WitHostBridge;

    impl crate::service::HostBridge for WitHostBridge {
        type Error = ();

        fn cancelled(&mut self) -> bool {
            host::cancelled()
        }

        fn call(&mut self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
            host::call(request).map_err(|_| ())
        }
    }

    struct Collections;

    impl Guest for Collections {
        fn descriptor() -> Result<ServiceDescriptor, ServiceError> {
            Ok(ServiceDescriptor {
                service_id: crate::service::SERVICE_ID.into(),
                methods: vec![crate::service::METHOD.into()],
            })
        }

        fn invoke(request: Vec<u8>) -> Result<Vec<u8>, ServiceError> {
            crate::service::invoke(&mut WitHostBridge, &request).map_err(|error| ServiceError {
                code: error.code().into(),
                message: error.message().into(),
            })
        }
    }

    export!(Collections);
}

#[cfg(test)]
mod policy_fixtures;
#[cfg(test)]
mod tests;
