//! Zero-authority candidate for the Vqro Collections package.
//!
//! This component deliberately advertises no methods and performs no host
//! calls. Collection behavior remains blocked on public M2 contracts.

#[cfg(any(target_arch = "wasm32", test))]
const SERVICE_ID: &str = "collections";
#[cfg(any(target_arch = "wasm32", test))]
const SERVICE_METHODS: &[&str] = &[];
#[cfg(any(target_arch = "wasm32", test))]
const NO_AUTHORITY_CODE: &str = "no_authority";
#[cfg(any(target_arch = "wasm32", test))]
const NO_AUTHORITY_MESSAGE: &str = "Collections candidate has no callable methods";

#[cfg(target_arch = "wasm32")]
mod guest {
    wit_bindgen::generate!({
        path: "../../wit/vqro-extension-service",
        world: "service",
    });

    struct CollectionsCandidate;

    impl Guest for CollectionsCandidate {
        fn descriptor() -> Result<ServiceDescriptor, ServiceError> {
            Ok(ServiceDescriptor {
                service_id: super::SERVICE_ID.into(),
                methods: super::SERVICE_METHODS
                    .iter()
                    .map(|method| (*method).to_string())
                    .collect(),
            })
        }

        fn invoke(_request: Vec<u8>) -> Result<Vec<u8>, ServiceError> {
            Err(ServiceError {
                code: super::NO_AUTHORITY_CODE.into(),
                message: super::NO_AUTHORITY_MESSAGE.into(),
            })
        }
    }

    export!(CollectionsCandidate);
}

#[cfg(test)]
mod tests {
    #[test]
    fn candidate_has_no_native_behavior() {
        // Product behavior must not be added before public M2 contracts land.
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.0.0");
        assert_eq!(super::SERVICE_ID, "collections");
        assert!(super::SERVICE_METHODS.is_empty());
        assert_eq!(super::NO_AUTHORITY_CODE, "no_authority");
        assert_eq!(
            super::NO_AUTHORITY_MESSAGE,
            "Collections candidate has no callable methods"
        );
    }
}
