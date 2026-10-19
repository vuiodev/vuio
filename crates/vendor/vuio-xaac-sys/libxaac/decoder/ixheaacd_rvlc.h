#ifndef IXHEAACD_RVLC_H
#define IXHEAACD_RVLC_H
void ixheaacd_rvlc_read(
    ia_bit_buf_struct *itt_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info);
IA_ERRORCODE ixheaacd_rvlc_dec(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_overlap_info *ptr_aac_dec_static_channel_info,
    ia_bit_buf_struct *itt_bit_buff);
void ixheaacd_hcr_read(ia_bit_buf_struct *itt_bit_buff,
                       ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
                       WORD32 ele_type);

void ixheaacd_carry_bit_branch_val(UWORD8 carry_bit, UWORD32 tree_node,
                                   UWORD32 *branch_val, UWORD32 *branch_node);

VOID ixheaacd_huff_sfb_table(WORD32 it_bit_buff, WORD16 *huff_index,
                             WORD32 *len, const UWORD16 *code_book_tbl,
                             const UWORD32 *idx_table);

#endif
