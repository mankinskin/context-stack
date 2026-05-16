/// Which kind of content the LLM should generate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GenerationMode {
    /// Generate voxel structures (coordinate + color lists).
    VoxelStructure = 0,
    /// Generate a WGSL SDF function for a custom shader effect.
    ShaderEffect = 1,
    /// Generate a spell modifier (shader + parameter tweaks).
    SpellModifier = 2,
}

impl GenerationMode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::VoxelStructure),
            1 => Some(Self::ShaderEffect),
            2 => Some(Self::SpellModifier),
            _ => None,
        }
    }

    /// Returns the system-prompt preamble for this mode.
    pub fn system_prompt_header(&self) -> &'static str {
        match self {
            Self::VoxelStructure => SYSTEM_PROMPT_VOXEL,
            Self::ShaderEffect => SYSTEM_PROMPT_SHADER,
            Self::SpellModifier => SYSTEM_PROMPT_SHADER,
        }
    }
}

const SYSTEM_PROMPT_VOXEL: &str = "\
You are a WebGPU world generator for a voxel sandbox game.\n\
MODE: voxel_structure\n\
Respond ONLY with JSON: { \"voxels\": [[x,y,z,color_hex], ...] }\n\
- Coordinates must be integers in range [-64, 64] relative to player position\n\
- color_hex is a 32-bit RGBA packed as \"0xRRGGBBAA\"\n\
- Maximum 4096 voxels per generation\n\
- Create the structure the user describes";

const SYSTEM_PROMPT_SHADER: &str = "\
You are a WebGPU world generator for a voxel sandbox game.\n\
MODE: shader_effect\n\
Respond ONLY with valid WGSL code defining a single SDF function:\n\
- Function signature: fn sd_custom(p: vec3<f32>, time: f32) -> f32\n\
- Use only: length, dot, cross, normalize, clamp, smoothstep, min, max, abs, sin, cos\n\
- NO texture sampling, NO buffer access, NO loops over external data\n\
- Return signed distance (negative = inside, positive = outside)";
