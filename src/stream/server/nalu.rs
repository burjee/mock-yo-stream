use bytes::{Bytes, Buf};

// Flv Data - Video Sequence_Header
// ------------------------| ----
// Version                 |   u8
// Profile Indication      |   u8
// Profile Compatability   |   u8
// Level Indication        |   u8
// Reserved                |   u6
// NALU Length             |   u2
// Reserved                |   u3
// SPS Count               |   u5
// SPS Length              |   u16
// SPS                     |   u[]
// PPS Count               |   u8
// PPS Length              |   u16
// PPS                     |   u[]
pub struct NaluConfig {
    pub version: u8,
    pub profile_indication: u8,
    pub profile_compatability: u8,
    pub level_indication: u8,
    pub nalu_size: u8,
    pub sps: Vec<Nalu>,
    pub pps: Vec<Nalu>,
}

impl NaluConfig {
    pub fn new() -> NaluConfig {
        NaluConfig {
            version: 0,
            profile_indication: 0,
            profile_compatability: 0,
            level_indication: 0,
            nalu_size: 0,
            sps: Vec::new(),
            pps: Vec::new(),
        }
    }

    pub fn set(&mut self, mut data: Bytes) {
        self.version = data.get_u8();
        self.profile_indication = data.get_u8();
        self.profile_compatability = data.get_u8();
        self.level_indication = data.get_u8();
        self.nalu_size = (data.get_u8() & 0b11) + 1;

        let sps_count = data.get_u8() & 0b11111;
        let mut sps = Vec::new();
        for _ in 0..sps_count {
            let sps_length = data.get_u16() as usize;
            let sps_temp = data.slice(..sps_length);
            data.advance(sps_length);
            sps.push(Nalu::read_unit(sps_temp));
        }

        let pps_count = data.get_u8();
        let mut pps = Vec::new();
        for _ in 0..pps_count {
            let pps_length = data.get_u16() as usize;
            let pps_temp = data.slice(..pps_length);
            data.advance(pps_length);
            pps.push(Nalu::read_unit(pps_temp));
        }

        self.sps = sps;
        self.pps = pps;
    }
}

// FLV Data Body
// ----------| --
// Nalu Type | u8
// RBSP      | []

// FLV Data Body Nalu Type
// -----| ---|
// F	| u1 |	forbidden zero bit, h.264 ????????????
// NRI	| u2 |	nal ref idc, ???0~3, I???/sps/pps???3, P??????2, B??????0
// Type	| u5 |	????????????
// -----------
// 0	  ?????????
// 1	  ????????????
// 2	  ?????????A
// 3	  ?????????B
// 4	  ?????????C
// 5	  ?????????
// 6	  ????????????????????????(SEI)
// 7	  SPS???????????????
// 8	  PPS???????????????
// 9	  ?????????
// 10	  ????????????
// 11	  ????????????
// 12	  ??????
// 13~23  ??????
// 24~31  ?????????

pub struct Nalu {
    pub ref_idc: u8,
    pub unit_type: u8,
    pub data: Bytes, // RBSP
}

impl Nalu {
    const INTER_DELIMITER: &'static [u8] = &[0x00, 0x00, 0x01];
    const BEGIN_DELIMITER: &'static [u8] = &[0x00, 0x00, 0x00, 0x01];
    const NALU_DELIMITER: &'static [u8] = &[0x00, 0x00, 0x00, 0x01, 0x09, 0x00];

    fn to_vec(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.data.len() + 1);

        let header = (self.ref_idc << 5) | (self.unit_type);
        v.push(header);
        v.extend(self.data.clone());
        v
    }

    pub fn read(mut data: Bytes, nalu_size: u8) -> Vec<Nalu> {
        let nalu_size = nalu_size as usize;
        let mut nal_units = Vec::new();

        while data.has_remaining() {
            let nalu_length = data.get_uint(nalu_size) as usize;
            let nalu_data = data.slice(..nalu_length);
            let nal_unit = Nalu::read_unit(nalu_data);
            data.advance(nalu_length);
            nal_units.push(nal_unit);
        }

        nal_units
    }

    pub fn read_unit(mut data: Bytes) -> Nalu {
        let nalu = data.get_u8();
        let ref_idc = (nalu >> 5) & 0x03;
        let unit_type = nalu & 0x1f;

        Nalu { ref_idc, unit_type, data }
    }

    // ??????es??????nalu header, ?????????0x00000001??????????????????0x000001????????????
    // ??????es???, pes???es???????????????type=9???nalu, ????????????????????????type=7???type=8???nalu, ??????nalu???????????????
    // Pes Header | nalu(0x09) | ??????(u8) | nalu(??????) | ?????? | nalu(0x67) | sps | nalu(0x68) | pps | nalu(0x65) | keyframe |
    // Pes Header | nalu(0x09) | ??????(u8) | nalu(??????) | ?????? | nalu(0x41) | ?????? |
    pub fn to_es_layer(nalu_config: &NaluConfig, data: Vec<Nalu>) -> Vec<u8> {
        let mut es = Vec::new();
        let mut is_delimit = false;
        let mut is_keyframe_delimit = false;

        for nalu in data {
            match nalu.unit_type {
                1 | 6 => {
                    if !is_delimit {
                        es.extend(Nalu::NALU_DELIMITER);
                        is_delimit = true;
                    }
                }
                5 => {
                    if !is_delimit {
                        es.extend(Nalu::NALU_DELIMITER);
                        is_delimit = true;
                    }

                    if !is_keyframe_delimit {
                        let nalu = nalu_config.sps.first().unwrap();
                        let sps: Vec<u8> = nalu.to_vec();
                        es.extend(Nalu::BEGIN_DELIMITER);
                        es.extend(sps);

                        let nalu = nalu_config.pps.first().unwrap();
                        let pps: Vec<u8> = nalu.to_vec();
                        es.extend(Nalu::BEGIN_DELIMITER);
                        es.extend(pps);

                        is_keyframe_delimit = true;
                    }
                }
                _ => continue,
            }

            es.extend(Self::INTER_DELIMITER);
            es.extend(nalu.to_vec());
        }
        es
    }
}
