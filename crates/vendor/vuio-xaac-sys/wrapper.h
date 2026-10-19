#include <stddef.h>

#include "libxaac/common/ixheaac_type_def.h"
#include "libxaac/common/ixheaac_error_standards.h"

#include "libxaac/decoder/ixheaacd_memory_standards.h"
#include "libxaac/decoder/ixheaacd_apicmd_standards.h"
#include "libxaac/decoder/ixheaacd_aac_config.h"
#include "libxaac/decoder/ixheaacd_interface.h"
#include "libxaac/decoder/drc_src/impd_apicmd_standards.h"

#include "libxaac/encoder/ixheaace_api.h"

IA_ERRORCODE ixheaacd_dec_api(pVOID p_ia_xheaac_dec_obj, WORD32 i_cmd, WORD32 i_idx,
                              pVOID pv_value);
IA_ERRORCODE ia_drc_dec_api(pVOID p_ia_drc_dec_obj, WORD32 i_cmd, WORD32 i_idx,
                            pVOID pv_value);
