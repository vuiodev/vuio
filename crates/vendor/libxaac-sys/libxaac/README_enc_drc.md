# 1. Introduction
libxaac supports inclusion of DRC metadata within encoded audio streams. When DRC is enabled via command-line switch, metadata is provided through `impd_drc_config_params` text file. Loudness leveling, as defined in amendment AMD-2 (ISO/IEC 23003-4:2020/Amd. 2:2023(E)), is also supported.

# 2. Input Structure
The following input structure describes how inputs are passed to encoder.

## 2.1 uniDRC configuration

### 2.1.1 DRC instructions
```plaintext
drc_instructions_uni_drc_count:(int)
└── downmix_id:(int)
└── drc_set_effect:(int)
└── gain_set_channels:(int)
    └──gain_set_index: (int)
└── num_drc_channel_groups:(int)
└── leveling_present: (int) (if drc_set_effect & 2048)
└── ducking_only_set_present: (int) (if drc_set_effect & 2048)
```
### 2.1.2 DRC coefficients
```plaintext
drc_coefficients_uni_drc_count:(int)
└── gain_set_count:(int)
    └── band_count:(int)
        └── nb_points:(int)
          └── x:(float)
          └── y:(float)
        └── width:(float)
        └── attack:(float)
        └── decay:(float)
        └── start_sub_band_index:(int)(if band_count > 1)
```
### 2.1.3 uniDRC config extension
```plaintext
uni_drc_config_ext_present: (int)
└── downmix_instructions_v1_present:(int)
    └── downmix_instructions_v1_count: (int)
        └── target_layout: (int)
            └── dwn_mix_coeff: (float)
    └── drc_coeffs_and_instructions_uni_drc_v1_present: (int)
        └── drc_coefficients_uni_drc_v1_count: (int)
            └── gain_set_count: (int)
                └── band_count: (int)
                    └── nb_points: (int)
                        └── x: (float)
                        └── y: (float)
                    └── width: (float)
                    └── attack: (float)
                    └── decay: (float)
                    └── start_sub_band_index:(int)(if band_count > 1)
        └── drc_instructions_uni_drc_v1_count: (int)
            └── downmix_id: (int)
            └── drc_set_effect: (int)
            └── gain_set_channels: (int)
                └── gain_set_index: (int)
            └── num_drc_channel_groups: (int)
            └── leveling_present: (int) (if drc_set_effect & 2048)
            └── ducking_only_set_present: (int) (if drc_set_effect & 2048)
```
### 2.1.4 Element description of uniDRC configuration parameters
Here is a brief description of each element in uniDRC configuration.
| **Sl. No.** | **Element name** | **Element description** | **Element type** | 
|-----|------|------|------|
| 1 |`drc_instructions_uni_drc_count:` | DRC instructions count. Valid values are 0 to 8. | Integer | 
| 2 |`drc_coefficients_uni_drc_count:` | DRC coefficients count. Valid values are 0 to 7. | Integer | 
| 3 |`uni_drc_config_ext_present:` | Flag to indicate if uniDRC config extension is present. Valid values are 0 and 1. | Integer | 
| 4 |`downmix_instructions_v1_present:` | Flag to indicate if downminx instructions v1 are present. Valid values are 0 and 1. | Integer | 
| 5 |`downmix_instructions_v1_count:` | Downminx instructions v1 count. Valid values are 0 to 16. | Integer | 
| 6 |`target_layout:` | Indicates the target speaker configuration for the downmixed output. | Integer | 
| 7 |`dwn_mix_coeff:` | Specifies gain applied to each input channel in the downmixed output. | Float | 
| 8 |`drc_coeffs_and_instructions_uni_drc_v1_present:` | Flag to indicate if uniDRCv1 intructions and coefficients are present. Valid values are 0 and 1. | Integer | 
| 9 |`drc_coefficients_uni_drc_v1_count:` | DRC coefficients v1 count. Valid values are 0 to 7. | Integer | 
| 10 |`gain_set_count:` | Gain sequences count. Valid values are 0 to 8. | Integer |
| 11 |`band_count:` | Band count in a gain sequence. Valid values are 0 to 8. | Integer |
| 12 |`nb_points:` | DRC curve points in a gain sequence. Valid values are 0 to 256. | Integer |
| 13 |`x:` | Coordinate of a DRC curve point indicating the input loudness (in dB). | Float |
| 14 |`y:` | Coordinate of a DRC curve point indicating the output loudness (in dB). | Float |
| 15 |`width:` | Determines the smoothness of the transition between curve points (in dB). | Float |
| 16 |`attack:` |  Determines gain curve’s response time after input level exceeds threshold value (in seconds). | Float |
| 17 |`decay:` | Determines gain curve’s response time after input level falls below threshold value (in seconds). | Float |
| 18 |`start_sub_band_index:` | Start sub band index of band count is more than `1`. | Integer |
| 19 |`drc_instructions_uni_drc_v1_count:` | DRC instructions v1 count. Valid values are 0 to 8. | Integer | 
| 20 |`downmix_id:` | Downmix ID of the drc set. | Integer |
| 21 |`drc_set_effect:` | Set DRC effect. | Integer |
| 22 |`gain_set_channels:` | Total number of channels. Can be equivalent to total channels in audio scene/ total channels in a group/ total channels in preset. | Integer |
| 23 |`gain_set_index:` | Gain Set index for indiviual channel. This is mapped to coeffecient set. | Integer |
| 24 |`num_drc_channel_groups:` | Total number of unique set groups.(Unique Set index). | Integer |
| 25 |`num_drc_channel_groups:` | Flag to indicate if loudness leveling is enabled. Applicable only if drc set effect has a ducking bit enabled. | Integer |
| 26 |`ducking_only_set_present:` | Flag to indicate if ducking only set is present. Applicable only if drc set effect has a ducking bit enabled. | Integer |


## 2.2 Loundness configuration

### 2.2.1 loudness info
```plaintext
loudness_info_count: (int)
└── drc_set_id: (int)
└── downmix_id: (int)
└── sample_peak_level_present: (int)
    └── sample_peak_level: (float)
└── true_peak_level_present: (int)
    └── true_peak_level: (float)
    └── true_peak_level_measurement_system: (int)
    └── true_peak_level_reliability: (int)
└── measurement_count: (int)
    └── method_definition: (int)
    └── method_value: (float)
    └── measurement_system: (int)
    └── reliability: (int)
```
### 2.2.2 loudness info album
```plaintext
loudness_info_album_count: (int)
└── drc_set_id: (int)
└── downmix_id: (int)
└── sample_peak_level_present: (int)
    └── sample_peak_level: (float)
└── true_peak_level_present: (int)
    └── true_peak_level: (float)
    └── true_peak_level_measurement_system: (int)
    └── true_peak_level_reliability: (int)
└── measurement_count: (int)
    └── method_definition: (int)
    └── method_value: (float)
    └── measurement_system: (int)
    └── reliability: (int)
```
### 2.2.3 loudness info set extension
```plaintext
loudness_info_set_ext_present: (int)
└── loudness_info_v1_album_count: (int)
    └── drc_set_id: (int)
    └── downmix_id: (int)
    └── sample_peak_level_present: (int)
        └── sample_peak_level: (float)
    └── true_peak_level_present: (int)
        └── true_peak_level: (float)
        └── true_peak_level_measurement_system: (int)
        └── true_peak_level_reliability: (int)
    └── measurement_count: (int)
        └── method_definition: (int)
        └── method_value: (float)
        └── measurement_system: (int)
        └── reliability: (int)
└── loudness_info_v1_count: (int)
    └── drc_set_id: (int)
    └── downmix_id: (int)
    └── sample_peak_level_present: (int)
        └── sample_peak_level: (float)
    └── true_peak_level_present: (int)
        └── true_peak_level: (float)
        └── true_peak_level_measurement_system: (int)
        └── true_peak_level_reliability: (int)
    └── measurement_count: (int)
        └── method_definition: (int)
        └── method_value: (float)
        └── measurement_system: (int)
        └── reliability: (int)
```
### 2.2.4 Element description of loudness configuration parameters
Here is a brief description of each element in loudness configuration.
| **Sl. No.** | **Element name** | **Element description** | **Element type** | 
|-----|------|------|------|
| 1 |`loudness_info_count:` | loudnessInfo count. Valid values are 0 to 31. | Integer | 
| 2 |`drc_set_id:` | Defines the DRC set relevant to the loudness data. | Integer |
| 3 |`downmix_id:` | Defines the downmix configuration relevant to the loudness data. | Integer |
| 4 |`sample_peak_level_present:` | Flag to indicate if sample peak level is present. Valid values are 0 and 1. | Integer |
| 5 |`sample_peak_level:` | Defines the maximum sample peak level in the audio signal (in dB.) | Float |
| 6 |`true_peak_level_present:` | Flag to indicate if sample peak level is present. Valid values are 0 and 1. | Integer |
| 7 |`true_peak_level:` | Defines the maximum true peak level of the audio signal (in dB). | Float |
| 8 |`true_peak_level_measurement_system:` |  Defines the measurement system used to determine the true peak level. | Integer |
| 9 |`true_peak_level_reliability:` | Defines the reliability level of the true peak measurement. | Integer |
| 10 |`measurement_count:` | Defines the number of measurements used to calculate the loudness value. Valid values are 0 to 14 | Integer |
| 11 |`method_definition:` | Defines the method used to measure loudness level. | Integer |
| 12 |`method_value:` | Defines the loudness level value associated with the measurement method. | Float |
| 13 |`measurement_system:` | Defines the measurement system employed to measure the loudness value. | Integer |
| 14 |`reliability:` | Defines the reliability level of the measured value. | Integer |


## 2.3 Interpretation of DRC gain curve
The Dynamic Range Control (DRC) feature in libxaac encoder adjusts audio volume frame by frame based on input loudness. It uses a user-defined curve that maps input levels to corresponding output levels. This curve is constructed using gain points, which determine when and how encoder applies gain or attenuation to ensure consistent listening experience.

### I/O Loudness Mapping Curve
The gain curve maps input loudness levels to desired output levels using a series of (x, y) points that form a piecewise linear function. For each DRC set, a gain sequence is defined using a set of gain curve points, provided in sequence under the uniDRCv1 coefficients. Encoder calculates the gain for each audio frame by interpolating between these points, enabling smooth and precise volume adjustments. This approach provides control over how input loudness is transformed, allowing compression settings to be customized for different types of audio content. The shape of the curve directly influences dynamic range behavior, affecting both perceived loudness and overall sound quality.

Following is an example with two DRC gain sets in the configuration file, showing how input loudness maps to output levels using separate gain curves:

```plaintext
########### uniDRCv1 configuration ###########
drc_coefficients_uni_drc_count_v1:1
gain_set_count:2
#gainset 0
band_count:1
nb_points:3
x:-60.0
y:-50.0
x:-30.0
y:-20.0
x:0.0
y:-5.0
width:0.01
attack:0.01
decay:20.0
#gainset 1
band_count:1
#gain parameters m=0
nb_points:3
x:-50.0
y:-35.0
x:-20.0
y:-20.0
x:-0.0
y:-15.0
width:0.01
attack:0.01
decay:20.0
drc_instructions_uni_drc_count_v1:2
#drc instruction 0
downmix_id:0
drc_set_effect:0x0001
gain_set_channels:2
gain_set_index:0
gain_set_index:0
num_drc_channel_groups:1
#drc instruction 0
downmix_id:0
drc_set_effect:0x800
gain_set_channels:2
gain_set_index:1
gain_set_index:1
num_drc_channel_groups:1
leveling_present:0
ducking_only_set_present:0
```
In the above example, gain curves for two DRC sets are defined using (x, y) points, where x represents input loudness levels and y represents the corresponding output loudness:
| **Input Loudness of drc set 1(x)** | **Output Loudness of drc set 1 (y)** | **Input Loudness of drc set 2(x)** | **Output Loudness of drc set 2 (y)** |
| ---------------------- | ----------------------- | ---------------------- | ----------------------- |
| -60.0 dB               | -50.0 dB                | -50.0 dB               | -35.0 dB                |
| -30.0 dB               | -20.0 dB                | -20.0 dB               | -20.0 dB                |
| 0.0 dB                 | -5.0 dB                 | 0.0 dB                 | -15.0 dB                |

These points specify how the encoder adjusts volume based on the loudness of each audio frame, using piecewise linear interpolation between (x, y) pairs.

For example, consider an audio signal with an input loudness of -18 dB. If only the first DRC set is applied by the decoder, the output loudness will be -24 dB. This is because -18 dB falls within the input range of -30 dB to 0 dB, where the gain curve increases output by 0.5 dB for every 1 dB increase in input, resulting in a total gain reduction of 6 dB. Similarly, if only the second DRC set is applied, the output loudness will be -19.5 dB, with the gain increasing by 0.25 dB per 1 dB input rise between -20 dB and 0 dB, resulting in a total gain reduction of 1.5 dB.

When both DRC sets are applied together by the decoder, their gain reductions combine for a total of 7.5 dB, producing a final output loudness of -25.5 dB.


Note: It is recommended to use exclusively the uniDRCv1 configuration to encode streams with loudness leveling enhancements as defined in Amendment 2. A sample DRC configuration for the encoder can be found in [`impd_drc_config_params.txt`](test/encoder/impd_drc_config_params.txt).

