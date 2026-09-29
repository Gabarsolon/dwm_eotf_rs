use aho_corasick::{AhoCorasick, MatchKind};
use anyhow::Result;
use bytemuck::{cast, cast_slice};
use shader_patcher::{BinaryPatcher, error::Error};
use tracing::{debug, warn};

static ORIGINAL_PATTERNS: [[f32; 4]; 4] = [
    [2.4, 2.4, 2.4, 0.0],
    [0.04045, 0.04045, 0.04045, 0.0],
    [0.055000, 0.055000, 0.055000, 0.0],
    [0.94786733, 0.94786733, 0.94786733, 0.0],
];

// All 6 forward SDR-to-scRGB conversion shaders across all DWM feature levels and shader bundles
static HASH_WHITELIST: [u128; 6] = [
    0x84cd59df5f46a95d22ee9bffed30de4c, // 4cde30edff9bee225da9465fdf59cd84 (RGB_FULL_G22_NONE_P709_To_scRGB, SM 4.0)
    0x6965d7ccd34ad1892d6b815cdea54270, // 7042a5de5c816b2d89d14ad3ccd76569 (RGB_FULL_G22_NONE_P709_IA_To_scRGB, SM 4.0)
    0x85d254ecdbd4d71dcdec559258d1e696, // 96e6d1589255eccd1dd7d4dbec54d285 (RGB_FULL_G22_NONE_P709_To_scRGB, SM 4.0 + Level 9)
    0x3caf9cdde6b655e3ddfba2c137b02621, // 2126b037c1a2fbdde355b6e6dd9caf3c (RGB_FULL_G22_NONE_P709_IA_To_scRGB, SM 4.0 + Level 9)
    0xdbadc38d66727c965df029e2ff26892c, // 2c8926ffe229f05d967c72668dc3addb (RGB_FULL_G22_NONE_P709_To_scRGB, SM 4.0 + Level 9)
    0xf5a79888be546336d9b324afbbbf93f6, // f693bfbbaf24b3d9366354be8898a7f5 (RGB_FULL_G22_NONE_P709_IA_To_scRGB, SM 4.0 + Level 9)
];

// All 15 DWM AlphaCorrectSDR / AlphaCorrectExtendedSDR / BoostSDRLuminance shaders
// In unpatched DWM, these apply a polynomial delta that clamps alpha to 0 for subtle
// translucent elements (taskbar borders, window frames, Settings Mica cards) when luminance > 1.0.
static ALPHA_CORRECT_WHITELIST: [u128; 15] = [
    0x758445033aff77cf29cee921baf880fa, // fa80f8ba21e9ce29cf77ff3a03458475
    0xa4c7098ffff49306e2903060139e3302, // 02339e13603090e20693f4ff8f09c7a4
    0x8409a2019f0244e55454452192a1773e, // 3e77a19221455454e544029f01a20984
    0x2f47c64cc12e299e8908a6559a6ab706, // 06b76a9a55a608899e292ec14cc6472f
    0xbedcfd5b561c32f89e25143a56d57153, // 5371d5563a14259ef8321c565bfddcbe
    0x4e7a85dd3ca31d26522d23665e029155, // 5591025e66232d52261da33cdd857a4e
    0xdf24f226d9b0aba18adfbd889fa024d4, // d424a09f88bddf8aa1abb0d926f224df
    0xa50ef1a70ad73ea497e3af2755c94d4d, // 4d4dc95527afe397a43ed70aa7f10ea5
    0x86cb33f844d10c92ccaa46b32242e68e, // 8ee64222b346aacc920cd144f833cb86
    0x74ab0d8e3e3ba4638c3f3ec97ff64d36, // 364df67fc93e3f8c63a43b3e8e0dab74
    0x8c6b41ab0271dcc371334ea7dd2d635f, // 5f632ddda74e3371c3dc7102ab416b8c
    0xc846a6eff08c0b5c2d944de6653fdd35, // 35dd3f65e64d942d5c0b8cf0efa646c8
    0xb50b1a06f22d392aa46b819c2e4d26c5, // c5264d2e9c816ba42a392df2061a0bb5
    0xb3fb6fd115c919e5497e9795957cc013, // 13c07c9595977e49e519c915d16ffbb3
    0x7737fc9ef8ff2f6711c3b81b49f7de0c, // 0cdef7491bb8c311672ffff89efc3777
];

// Polynomial coefficients in AlphaCorrectSDR / AlphaCorrectExtendedSDR
const ALPHA_B1: [u8; 4] = [0xac, 0xa4, 0xe3, 0xbf]; // -1.7784628868f32
const ALPHA_B2: [u8; 4] = [0x27, 0x55, 0xea, 0xbd]; // -0.1144202277f32

// Polynomial coefficients in BoostSDRLuminance
const ALPHA_C1: [u8; 4] = [0xd7, 0x12, 0xca, 0xbf]; // -1.57869995f32
const ALPHA_C2: [u8; 4] = [0x17, 0xb7, 0xd1, 0x3c]; //  0.025600f32
const ALPHA_C3: [u8; 4] = [0x60, 0xe5, 0xd0, 0xbd]; // -0.102000f32
const ALPHA_C4: [u8; 4] = [0x78, 0x7a, 0x55, 0x3f]; //  0.833899975f32

const ZERO4: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

fn contains_bytes(data: &[u8], sub: &[u8]) -> bool {
    data.windows(sub.len()).any(|w| w == sub)
}

fn replace_all_occurrences(data: &mut [u8], target: &[u8], replacement: &[u8]) -> usize {
    assert_eq!(target.len(), replacement.len());
    let t_len = target.len();
    let mut count = 0;
    let mut i = 0;
    while i + t_len <= data.len() {
        if &data[i..i + t_len] == target {
            data[i..i + t_len].copy_from_slice(replacement);
            count += 1;
            i += t_len;
        } else {
            i += 1;
        }
    }
    count
}

pub struct SimplePatcher<'a> {
    aho: &'a AhoCorasick,
    replacements: [[u8; 16]; 4],
    ignore_whitelist: bool,
    fix_borders: bool,
}

impl<'a> SimplePatcher<'a> {
    pub fn new(
        aho: &'a AhoCorasick,
        mut gamma: f32,
        mut brightness: f32,
        ignore_whitelist: bool,
        fix_borders: bool,
    ) -> Self {
        if gamma <= 0.0 {
            warn!("Gamma must be positive, defaulting to 2.2!");
            gamma = 2.2;
        }

        if brightness <= 0.0 {
            warn!("Brightness must be positive, defaulting to 1.0!");
            brightness = 1.0;
        }

        // Proper linear luminance (nits) factor:
        // In the EOTF shader, the formula is (c * scale)^gamma = c^gamma * scale^gamma.
        // For scale^gamma == brightness (linear multiplier), scale must be brightness^(1.0 / gamma).
        let scale = brightness.powf(1.0 / gamma);

        let replacements: [[u8; 16]; 4] = cast([
            [gamma, gamma, gamma, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [scale, scale, scale, 0.0],
        ]);

        Self {
            aho,
            replacements,
            ignore_whitelist,
            fix_borders,
        }
    }
}

impl<'a> BinaryPatcher for SimplePatcher<'a> {
    fn patch(&self, data: &mut [u8], checksum: u128) -> Result<bool, Error> {
        // 1. Check if this is a forward SDR -> scRGB EOTF shader
        let is_eotf = HASH_WHITELIST.contains(&checksum)
            || contains_bytes(data, b"RGB_FULL_G22_NONE_P709_To_scRGB")
            || contains_bytes(data, b"RGB_FULL_G22_NONE_P709_IA_To_scRGB")
            || (self.ignore_whitelist && contains_bytes(data, &cast::<[f32; 4], [u8; 16]>(ORIGINAL_PATTERNS[0])));

        if is_eotf {
            let patched = self.aho.replace_all_bytes(data, &self.replacements);

            if patched.len() == data.len() {
                if patched != data {
                    data.copy_from_slice(&patched);
                    return Ok(true);
                }
            } else {
                return Err(Error::ReplLenChange);
            }
        }

        // 2. Check if this is an AlphaCorrect / BoostSDR shader that corrupts translucent borders
        if self.fix_borders {
            let is_alpha_shader = ALPHA_CORRECT_WHITELIST.contains(&checksum)
                || (contains_bytes(data, &ALPHA_B1) && contains_bytes(data, &ALPHA_B2))
                || (contains_bytes(data, &ALPHA_C1) && contains_bytes(data, &ALPHA_C4));

            if is_alpha_shader {
                let mut count = 0;
                count += replace_all_occurrences(data, &ALPHA_B1, &ZERO4);
                count += replace_all_occurrences(data, &ALPHA_B2, &ZERO4);
                count += replace_all_occurrences(data, &ALPHA_C1, &ZERO4);
                count += replace_all_occurrences(data, &ALPHA_C2, &ZERO4);
                count += replace_all_occurrences(data, &ALPHA_C3, &ZERO4);
                count += replace_all_occurrences(data, &ALPHA_C4, &ZERO4);

                if count > 0 {
                    debug!(
                        "Neutralized {} alpha correction coefficients in shader `{:032x}`",
                        count, checksum
                    );
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}

pub fn build_aho_corasick() -> Result<AhoCorasick> {
    let patterns: &[[u8; 16]] = cast_slice(&ORIGINAL_PATTERNS);

    Ok(AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(patterns)?)
}
