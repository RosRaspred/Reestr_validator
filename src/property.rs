use exonum::encoding::Field;
use exonum::crypto::{PublicKey, Hash};

encoding_struct! {
    struct Property {
        const SIZE = 88;

        field property_id:            &PublicKey  [00 => 32]
        // field prev_property_tx:       &Hash       [32 => 64]
        field registrator_id:         &PublicKey  [64 => 72]
        field object_value:           u64         [72 => 80]
        field owner_name:             &str        [80 => 88]
        // field owner_evidence:               &str        [88 => 96] 
        field status:                 u64         [96 => 104]
    }
}

impl Property {
    pub fn changeStatus(self, new_status: u64) -> Self {
        let status = new_status;
        Self::new(self.property_id(), self.registrator_id(), self.object_value(), self.owner_name(), new_status)
    }
}