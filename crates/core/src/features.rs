use crate::settings::AppSettings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureId {
    LiveScenes,
    AudioEffectsStudio,
    SmartSoundSearch,
    AdvancedAudioRouting,
    MacroSoundboard,
    MiniMode,
    AiSoundAssistant,
    CommunitySoundPacks,
    VoiceChanger,
    RadialMenu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureAvailability {
    Available,
    Beta,
    ComingSoon,
}

pub struct FeatureDefinition {
    pub id: FeatureId,
    pub default_enabled: bool,
    pub availability: FeatureAvailability,
}

pub const ALL_FEATURES: &[FeatureDefinition] = &[
    FeatureDefinition { id: FeatureId::LiveScenes, default_enabled: true, availability: FeatureAvailability::ComingSoon },
    FeatureDefinition { id: FeatureId::AudioEffectsStudio, default_enabled: true, availability: FeatureAvailability::ComingSoon },
    FeatureDefinition { id: FeatureId::SmartSoundSearch, default_enabled: true, availability: FeatureAvailability::Beta },
    FeatureDefinition { id: FeatureId::AdvancedAudioRouting, default_enabled: true, availability: FeatureAvailability::ComingSoon },
    FeatureDefinition { id: FeatureId::MacroSoundboard, default_enabled: true, availability: FeatureAvailability::ComingSoon },
    FeatureDefinition { id: FeatureId::MiniMode, default_enabled: true, availability: FeatureAvailability::ComingSoon },
    FeatureDefinition { id: FeatureId::AiSoundAssistant, default_enabled: false, availability: FeatureAvailability::ComingSoon },
    FeatureDefinition { id: FeatureId::CommunitySoundPacks, default_enabled: true, availability: FeatureAvailability::ComingSoon },
    FeatureDefinition { id: FeatureId::VoiceChanger, default_enabled: true, availability: FeatureAvailability::Available },
    FeatureDefinition { id: FeatureId::RadialMenu, default_enabled: true, availability: FeatureAvailability::Available },
];

impl FeatureId {
    pub fn key(self) -> &'static str {
        match self {
            Self::LiveScenes => "LiveScenes",
            Self::AudioEffectsStudio => "AudioEffectsStudio",
            Self::SmartSoundSearch => "SmartSoundSearch",
            Self::AdvancedAudioRouting => "AdvancedAudioRouting",
            Self::MacroSoundboard => "MacroSoundboard",
            Self::MiniMode => "MiniMode",
            Self::AiSoundAssistant => "AiSoundAssistant",
            Self::CommunitySoundPacks => "CommunitySoundPacks",
            Self::VoiceChanger => "VoiceChanger",
            Self::RadialMenu => "RadialMenu",
        }
    }

    pub fn name_key(self) -> &'static str {
        match self {
            Self::LiveScenes => "Feature_LiveScenes_Name",
            Self::AudioEffectsStudio => "Feature_AudioEffectsStudio_Name",
            Self::SmartSoundSearch => "Feature_SmartSoundSearch_Name",
            Self::AdvancedAudioRouting => "Feature_AdvancedAudioRouting_Name",
            Self::MacroSoundboard => "Feature_MacroSoundboard_Name",
            Self::MiniMode => "Feature_MiniMode_Name",
            Self::AiSoundAssistant => "Feature_AiSoundAssistant_Name",
            Self::CommunitySoundPacks => "Feature_CommunitySoundPacks_Name",
            Self::VoiceChanger => "Feature_VoiceChanger_Name",
            Self::RadialMenu => "Feature_RadialMenu_Name",
        }
    }

    pub fn desc_key(self) -> &'static str {
        match self {
            Self::LiveScenes => "Feature_LiveScenes_Desc",
            Self::AudioEffectsStudio => "Feature_AudioEffectsStudio_Desc",
            Self::SmartSoundSearch => "Feature_SmartSoundSearch_Desc",
            Self::AdvancedAudioRouting => "Feature_AdvancedAudioRouting_Desc",
            Self::MacroSoundboard => "Feature_MacroSoundboard_Desc",
            Self::MiniMode => "Feature_MiniMode_Desc",
            Self::AiSoundAssistant => "Feature_AiSoundAssistant_Desc",
            Self::CommunitySoundPacks => "Feature_CommunitySoundPacks_Desc",
            Self::VoiceChanger => "Feature_VoiceChanger_Desc",
            Self::RadialMenu => "Feature_RadialMenu_Desc",
        }
    }
}

pub fn definition(id: FeatureId) -> &'static FeatureDefinition {
    ALL_FEATURES
        .iter()
        .find(|d| d.id == id)
        .expect("feature definition")
}

pub fn is_enabled(settings: &AppSettings, id: FeatureId) -> bool {
    settings
        .features
        .get(id.key())
        .copied()
        .unwrap_or_else(|| definition(id).default_enabled)
}

pub fn set_enabled(settings: &mut AppSettings, id: FeatureId, enabled: bool) {
    settings.features.insert(id.key().to_string(), enabled);
}
