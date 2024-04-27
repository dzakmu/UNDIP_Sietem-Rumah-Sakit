#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct PatientRecord {
    name: String,
    id: u64,
    complaint: String,
}

impl Storable for PatientRecord {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for PatientRecord {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static PATIENT_RECORDS: RefCell<StableBTreeMap<u64, PatientRecord, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct PatientRecordPayload {
    name: String,
    complaint: String,
}

#[ic_cdk::query]
fn get_patient_record(id: u64) -> Result<PatientRecord, Error> {
    match _get_patient_record(&id) {
        Some(record) => Ok(record),
        None => Err(Error::NotFound {
            msg: format!("Patient record with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_patient_record(record: PatientRecordPayload) -> Option<PatientRecord> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");
    let patient_record = PatientRecord {
        id,
        name: record.name,
        complaint: record.complaint,
    };
    do_insert(&patient_record);
    Some(patient_record)
}

#[ic_cdk::update]
fn update_patient_record(id: u64, payload: PatientRecordPayload) -> Result<PatientRecord, Error> {
    match PATIENT_RECORDS.with(|service| service.borrow().get(&id)) {
        Some(mut record) => {
            record.complaint = payload.complaint;
            record.name = payload.name;
            do_insert(&record);
            Ok(record)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "Couldn't update patient record with id={}. Record not found",
                id
            ),
        }),
    }
}

fn do_insert(record: &PatientRecord) {
    PATIENT_RECORDS.with(|service| service.borrow_mut().insert(record.id, record.clone()));
}

#[ic_cdk::update]
fn delete_patient_record(id: u64) -> Result<PatientRecord, Error> {
    match PATIENT_RECORDS.with(|service| service.borrow_mut().remove(&id)) {
        Some(record) => Ok(record),
        None => Err(Error::NotFound {
            msg: format!(
                "Couldn't delete patient record with id={}. Record not found.",
                id
            ),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

fn _get_patient_record(id: &u64) -> Option<PatientRecord> {
    PATIENT_RECORDS.with(|service| service.borrow().get(id))
}

ic_cdk::export_candid!();