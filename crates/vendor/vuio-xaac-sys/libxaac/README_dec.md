

# Introduction to libxaac decoder APIs

## Decoder APIs

A single API is used to get and set configurations and execute the decode thread, based on command index passed.
* ia_xheaacd_dec_api

| **API Command** | **API Sub Command** | **Description** |
|------|------|------|
|IA_API_CMD_GET_LIB_ID_STRINGS | IA_CMD_TYPE_LIB_NAME | Gets the decoder library name |
|IA_API_CMD_GET_LIB_ID_STRINGS | IA_CMD_TYPE_LIB_VERSION | Gets the decoder version |
|IA_API_CMD_GET_API_SIZE | 0 | Gets the memory requirements size of the API |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_API_PRE_CONFIG_PARAMS | Sets the configuration parameters of the libxaac decoder to default values |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_API_POST_CONFIG_PARAMS | Sets the attributes(size, priority, alignment) of all memory types required by the application onto the memory structure |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_PROCESS | Search for the valid header, does header decode to get the parameters and initializes state and configuration structure |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_DONE_QUERY | Checks if the initialization process has completed |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_PCM_WDSZ | Sets the bit width of the output PCM samples. The value has to be 16 |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_SAMP_FREQ | Sets the core AAC sampling frequency for RAW header decoding |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE | Sets the value of DRC effect type |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS | Sets the value of DRC target loudness |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DOWNMIX | Sets the parameter whether the output needs to be down-mix to mono(1) or not(0) |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_TOSTEREO | Sets the flag to disable interleave mono to stereo |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DSAMPLE | Sets the parameter whether the output needs to be downsampled(1) or not(0) |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_ISMP4 | Sets the flag to 0 or 1 to indicate whether given test vector is an mp4 file or not |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_MAX_CHANNEL | Sets the maximum number of channels present. Its maximum value is 2 for stereo library and 8 for multichannel library |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_COUP_CHANNEL | Sets the number of coupling channels to be used for coupling. It can take values from 0 to 16. This command is supported only if the library has multichannel support |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DOWNMIX_STEREO | Sets the flag of downmixing n number of channels to stereo. Can be 0 or 1. This command is supported only if the library has multichannel support |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_DISABLE_SYNC | Sets the flag of ADTS syncing or not ADTS syncing as 0 or 1 |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_AUTO_SBR_UPSAMPLE | Sets the parameter auto SBR upsample to 0 or 1. Used in case of stream changing from SBR present to SBR not present |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_CUT | Sets the value of DRC cut factor |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_BOOST | Sets the value of DRC boost factor |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_HEAVY_COMP | Sets the parameter to either enable/disable DRC heavy compression |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_FRAMESIZE | Sets the parameter whether decoder should decode for frame length 480 or 512 |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_LD_TESTING | Sets the flag to enable LD testing in decoder |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_HQ_ESBR | Sets the flag to enable/disable high quality eSBR |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_PS_ENABLE | Sets the flag to indicate the presence of PS data in eSBR bit stream |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_PEAK_LIMITER | Sets the flag to disable/enable peak limiter |
|IA_API_CMD_SET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_FRAMELENGTH_FLAG | Sets to flag to indicate whether decoder should decode for frame length 960 or 1024 |
|IA_API_CMD_SET_CONFIG_PARAM | IA_XHEAAC_DEC_CONFIG_PARAM_DRC_TARGET_LEVEL | Sets the value of DRC target level |
|IA_API_CMD_SET_CONFIG_PARAM | IA_XHEAAC_DEC_CONFIG_ERROR_CONCEALMENT | Sets the flag to disable/enable error concealment |
|IA_API_CMD_SET_CONFIG_PARAM | IA_XHEAAC_DEC_CONFIG_PARAM_ESBR | Sets the flag to disable/enable eSBR |
|IA_API_CMD_GET_N_MEMTABS | 0 | Gets the number of memory types |
|IA_API_CMD_GET_N_TABLES | 0 | Gets the number of tables |
|IA_API_CMD_GET_MEM_INFO_SIZE | 0 | Gets the size of the memory type being referred to by the index |
|IA_API_CMD_GET_MEM_INFO_ALIGNMENT | 0 | Gets the alignment information of the memory-type being referred to by the index |
|IA_API_CMD_GET_MEM_INFO_TYPE | 0 | Gets the type of memory being referred to by the index |
|IA_API_CMD_SET_MEM_PTR | 0 | Sets the pointer to the memory being referred to by the index to the input value |
|IA_API_CMD_GET_TABLE_INFO_SIZE | 0 | Gets the size of the memory type being referred to by the index |
|IA_API_CMD_GET_TABLE_INFO_ALIGNMENT | 0 | Gets the alignment information of the memory-type being referred to by the index |
|IA_API_CMD_GET_TABLE_PTR | 0 | Gets the address of the current location of the table |
|IA_API_CMD_SET_TABLE_PTR | 0 | Sets the relocated table address |
|IA_API_CMD_GET_MEMTABS_SIZE | 0 | Gets the size of the memory structures |
|IA_API_CMD_SET_MEMTABS_PTR | 0 | Sets the memory structure pointer in the library to the allocated value |
|IA_API_CMD_INPUT_OVER | 0 | Signals the end of bit-stream to the library |
|IA_API_CMD_SET_INPUT_BYTES | 0 | Sets the number of bytes available in the input buffer for initialization |
|IA_API_CMD_GET_CURIDX_INPUT_BUF | 0 | Gets the number of input buffer bytes consumed by the last initialization |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_PCM_WDSZ | Gets the output PCM word size |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_SAMP_FREQ | Gets the sampling frequency | 
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_NUM_CHANNELS | Gets the output number of channels |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_CHANNEL_MASK | Gets the channel mask |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_CHANNEL_MODE | Gets the channel mode. (Mono or PS/Stereo/Dual-mono) |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_SBR_MODE | Gets the SBR mode (Present/ Not Present) |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE | Gets the value of DRC effect type |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS | Gets the value of DRC target loudness |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_LOUD_NORM | Gets the value of DRC loudness normalization level |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_PARAM_AOT | Gets the value of audio object type |
|IA_API_CMD_GET_CONFIG_PARAM | IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUDNESS_LEVELING | Gets the value of loudness leveling flag |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_PTR | Gets the extension element pointer |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_BUF_SIZES | Gets the extension element buffer sizes |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_NUM_ELE | Gets the number of configuration elements |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_NUM_CONFIG_EXT | Gets the number of extension elements |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_LEN | Gets the gain payload length |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_BUF | Gets the gain payload buffer |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_CONFIG_GET_NUM_PRE_ROLL_FRAMES | Gets the number of preroll frames |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_DRC_IS_CONFIG_CHANGED | Gets the value of DRC config change flag |
|IA_API_CMD_GET_CONFIG_PARAM | IA_ENHAACPLUS_DEC_DRC_APPLY_CROSSFADE | Gets the value of DRC crossfade flag |
|IA_API_CMD_EXECUTE | IA_CMD_TYPE_DO_EXECUTE | Executes the decode thread |
|IA_API_CMD_EXECUTE | IA_CMD_TYPE_DONE_QUERY | Checks if the end of decode has been reached |
|IA_API_CMD_GET_OUTPUT_BYTES | 0 | Gets the number of bytes output by the decoder in the last frame |

### Aliases for some of the macros exist as XHEAAC

| **Macro** | **Alias** |
|------|------|
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_PCM_WDSZ | IA_XHEAAC_DEC_CONFIG_PARAM_PCM_WDSZ |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_SAMP_FREQ | IA_XHEAAC_DEC_CONFIG_PARAM_SAMP_FREQ |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_NUM_CHANNELS | IA_XHEAAC_DEC_CONFIG_PARAM_NUM_CHANNELS |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_CHANNEL_MASK | IA_XHEAAC_DEC_CONFIG_PARAM_CHANNEL_MASK |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_CHANNEL_MODE | IA_XHEAAC_DEC_CONFIG_PARAM_CHANNEL_MODE |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_SBR_MODE | IA_XHEAAC_DEC_CONFIG_PARAM_SBR_MODE |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE | IA_XHEAAC_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS | IA_XHEAAC_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_LOUD_NORM | IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUD_NORM |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DOWNMIX | IA_XHEAAC_DEC_CONFIG_PARAM_DOWNMIX |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_TOSTEREO | IA_XHEAAC_DEC_CONFIG_PARAM_TOSTEREO |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DSAMPLE | IA_XHEAAC_DEC_CONFIG_PARAM_DSAMPLE |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_ISMP4 | IA_XHEAAC_DEC_CONFIG_PARAM_MP4FLAG |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_MAX_CHANNEL | IA_XHEAAC_DEC_CONFIG_PARAM_MAX_CHANNEL |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_COUP_CHANNEL | IA_XHEAAC_DEC_CONFIG_PARAM_COUP_CHANNEL |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DOWNMIX_STEREO | IA_XHEAAC_DEC_CONFIG_PARAM_DOWNMIX_STEREO |
|IA_ENHAACPLUS_DEC_CONFIG_DISABLE_SYNC | IA_XHEAAC_DEC_CONFIG_DISABLE_SYNC |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_AUTO_SBR_UPSAMPLE | IA_XHEAAC_DEC_CONFIG_PARAM_AUTO_SBR_UPSAMPLE |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_CUT | IA_XHEAAC_DEC_CONFIG_PARAM_DRC_CUT |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_BOOST | IA_XHEAAC_DEC_CONFIG_PARAM_DRC_BOOST |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_HEAVY_COMP | IA_XHEAAC_DEC_CONFIG_PARAM_DRC_HEAVY_COMP |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_FRAMESIZE | IA_XHEAAC_DEC_CONFIG_PARAM_FRAMESIZE |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_LD_TESTING | IA_XHEAAC_DEC_CONFIG_PARAM_LD_TESTING |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_HQ_ESBR | IA_XHEAAC_DEC_CONFIG_PARAM_HQ_ESBR |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_PS_ENABLE | IA_XHEAAC_DEC_CONFIG_PARAM_PS_ENABLE |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_AOT | IA_XHEAAC_DEC_CONFIG_PARAM_AOT |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_PEAK_LIMITER | IA_XHEAAC_DEC_CONFIG_PARAM_PEAK_LIMITER |
|IA_ENHAACPLUS_DEC_CONFIG_PARAM_FRAMELENGTH_FLAG | IA_XHEAAC_DEC_CONFIG_PARAM_FRAMELENGTH_FLAG |

## DRC APIs

A single API is used to get and set configurations and execute the decode thread, based on command index passed.
* ia_drc_dec_api

| **API Command** | **API Sub Command** | **Description** |
|------|------|------|
|IA_API_CMD_GET_API_SIZE | 0 | Gets the memory requirements size of the API |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_API_PRE_CONFIG_PARAMS | Sets the configuration parameters of the libxaac decoder to default values |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_API_POST_CONFIG_PARAMS | Sets the attributes(size, priority, alignment) of all memory types required by the application onto the memory structure |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_PROCESS | Search for the valid header, does header decode to get the parameters and initializes state and configuration structure |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_CPY_BSF_BUFF | Sets the bitstream split format buffer |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_CPY_IC_BSF_BUFF | Sets the configuration bitstream split format buffer |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_CPY_IL_BSF_BUFF | Sets the loudness bitstream split format buffer |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_CPY_IN_BSF_BUFF | Sets the interface bitstream split format buffer |
|IA_API_CMD_INIT | IA_CMD_TYPE_INIT_SET_BUFF_PTR | Sets the input buffer pointer |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_SAMP_FREQ | Sets the sampling frequency of the input stream/data |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS | Sets the number of channels in the input stream/data |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_PCM_WDSZ | Sets the PCM word size of the input data |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT | Sets the bit stream format |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_INT_PRESENT | Sets the DRC decoders interface present flag to 1 or 0 |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_FRAME_SIZE | Sets the frame size |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_DRC_EFFECT_TYPE | Sets the value of DRC effect type |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_DRC_TARGET_LOUDNESS | Sets the value of DRC target loudness |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_DRC_LOUD_NORM | Sets the value of DRC loudness normalization level |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_APPLY_CROSSFADE | Sets the value of DRC crossfade flag |
|IA_API_CMD_SET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_CONFIG_CHANGED | Sets the value of DRC config change flag |
|IA_API_CMD_GET_MEM_INFO_SIZE | 0 | Gets the size of the memory type being referred to by the index |
|IA_API_CMD_GET_MEMTABS_SIZE | 0 | Gets the size of the memory structures |
|IA_API_CMD_SET_MEMTABS_PTR | 0 | Sets the memory structure pointer in the library to the allocated value |
|IA_API_CMD_GET_N_MEMTABS | 0 | Gets the number of memory types |
|IA_API_CMD_SET_INPUT_BYTES | 0 | Sets the number of bytes available in the input buffer for initialization |
|IA_API_CMD_GET_MEM_INFO_ALIGNMENT | 0 | Gets the alignment information of the memory-type being referred to by the index |
|IA_API_CMD_GET_MEM_INFO_TYPE | 0 | Gets the type of memory being referred to by the index |
|IA_API_CMD_SET_MEM_PTR | 0 | Sets the pointer to the memory being referred to by the index to the input value |
|IA_API_CMD_SET_INPUT_BYTES_BS | 0 | Sets the number of bytes to be processed in bitstream split format |
|IA_API_CMD_SET_INPUT_BYTES_IC_BS | 0 | Sets the number of bytes to be processed in configuration bitstream split format |
|IA_API_CMD_SET_INPUT_BYTES_IL_BS | 0 | Sets the number of bytes to be processed in loudness bitstream split format |
|IA_API_CMD_GET_CONFIG_PARAM | IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS | Gets the output number of channels |
|IA_API_CMD_EXECUTE | IA_CMD_TYPE_DO_EXECUTE | Executes the decode thread |

## Flowchart of calling sequence

![API Flowchart](docs/Api_flowchart_dec.png)

# Running the libxaac decoder

The libxaac decoder can be run by providing command-line parameters(CLI options) directly or by providing a parameter file as a command line argument.

Command line usage : 
```
<executable> -ifile:<input_file> -imeta:<meta_data_file> -ofile:<output_file> [options]

[options] can be,
[-mp4:<mp4_flag>]
[-pcmsz:<pcmwordsize>]
[-dmix:<down_mix>]
[-esbr_hq:<esbr_hq_flag>]
[-esbr_ps:<esbr_ps_flag>]
[-tostereo:<interleave_to_stereo>]
[-dsample:<down_sample_sbr>]
[-drc_cut_fac:<drc_cut_factor>]
[-drc_boost_fac:<drc_boost_factor>]
[-drc_target_level:<drc_target_level>]
[-drc_heavy_comp:<drc_heavy_compression>]
[-effect:<effect_type>]
[-target_loudness:<target_loudness>]
[-nosync:<disable_sync>]
[-sbrup:<auto_sbr_upsample>]
[-flflag:<framelength_flag>}
[-fs:<RAW_sample_rate>]
[-maxchannel:<maximum_num_channels>]
[-coupchannel:<coupling_channel>]
[-downmix:<down_mix_stereo>]
[-fs480:<ld_frame_size>]
[-ld_testing:<ld_testing_flag>]
[-peak_limiter_off:<peak_limiter_off_flag>]
[-err_conceal:<error_concealment_flag>]
[-esbr:<esbr_flag>]
[-loudness_leveling:<loudness_leveling_flag>]

where,
  <input_file>             is the input AAC-LC/HE-AACv1/HE-AACv2/AAC-LD/AAC-ELD/AAC-ELDv2/USAC file name.
  <meta_data_file>         is a text file which contains metadata. To be given when -mp4:1 is enabled.
  <output_file>            is the output file name.
  <mp4_flag>               is a flag that should be set to 1 when passing raw stream along with meta data text file.
  <pcmwordsize>            is the bits per sample info. value can be 16 or 24.
  <down_mix>               is to enable/disable always mono output. Default 1.
  <esbr_hq_flag>           is to enable/disable high quality eSBR. Default 0.
  <esbr_ps_flag>           is to indicate eSBR with PS. Default 0.
  <interleave_to_stereo>   is to enable/disable always interleaved to stereo output. Default 1.
  <down_sample_sbr>        is to enable/disable down-sampled SBR output. Default auto identification from header.
  <drc_cut_factor>         is to set DRC cut factor value. Default value is 1.
  <drc_boost_factor>       is to set DRC boost factor. Default value is 1.
  <drc_target_level>       is to set DRC target reference level. Default value is 108.
  <drc_heavy_compression>  is to enable/disable DRC heavy compression. Default value is 0.
  <effect_type>            is to set DRC effect type. Default value is 0.
  <target_loudness>        is to set target loudness level. Default value is -24.
  <disable_sync>           is to disable the ADTS/ADIF sync search i.e when enabled the decoder expects the header to be at the start of input buffer. Default 0.
  <auto_sbr_upsample>      is to enable(1) or disable(0) auto SBR upsample in case of stream changing from SBR present to SBR not present. Default 1.
  <framelength_flag>       is flag for decoding framelength of 1024 or 960. 1 to decode 960 frame length, 0 to decode 1024 frame length.
                           Frame length value in the GA header will override this option. Default 0.
  <RAW_sample_rate>        is to indicate the core AAC sample rate for a RAW stream. If this is specified no other file format headers are searched for.
  <maximum_num_channels>   is the number of maxiumum channels the input may have. Default is 6 for multichannel libraries and 2 for stereo libraries.
  <coupling_channel>       is element instance tag of independent coupling channel to be mixed. Default is 0.
  <down_mix_stereo>        is flag for Downmix. Give 1 to get stereo (downmix) output. Default is 0.
  <ld_frame_size>          is to indicate ld frame size. 0 is for 512 frame length, 1 is for 480 frame length. Default value is 512 (0).
  <ld_testing_flag>        is to enable/disable ld decoder testing. Default value is 0.
  <peak_limiter_off_flag>  is to enable/disable peak limiter. Default value is 0.
  <error_concealment_flag> is to enable/disable error concealment. Default value is 0.
  <esbr_flag>              is to enable/disable eSBR. Default value is 1.
  <loudness_leveling_flag> is to enable / disable loudness leveling. Default value is 1.

```
Note: `loudness_leveling_flag` is applicable only if `LOUDNESS_LEVELING_SUPPORT` macro is enabled.
Sample CLI:
```
<xaac_dec_exe> -ifile:in_file.aac -ofile:out_file.wav -pcmsz:16
```

# Validating the libxaac decoder

Conformance testing for AAC/HE-AACv1/HE-AACv2 mainly involves comparing 
decoder under test output with the ISO and 3GPP reference decoded output.

Testing for USAC is done using encoded streams generated using ISO USAC 
reference encoder. The output generated by libxaac USAC decoder is 
compared against the output generated by ISO USAC decoder for 16-bit 
conformance on the respective(ARMv7, ARMv8, X86_32, X86_64) platforms.

