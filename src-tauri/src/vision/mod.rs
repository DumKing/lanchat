pub mod activation;
pub mod alert;
pub mod manifest;
pub mod matching;
pub mod model_manager;
pub mod protocol;
pub mod registry;
pub mod runtime;
pub mod storage;
pub mod tracking;
pub mod types;
pub mod worker;

#[cfg(test)]
mod activation_tests;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod model_manager_tests;
#[cfg(test)]
mod people_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod recognition_tests;
#[cfg(test)]
mod registry_tests;
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod storage_tests;
#[cfg(test)]
mod types_tests;
#[cfg(test)]
mod worker_tests;
