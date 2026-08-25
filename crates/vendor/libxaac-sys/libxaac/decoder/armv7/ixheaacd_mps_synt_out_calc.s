@ * limitations under the License.
@ *
@ *****************************************************************************
@ * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
@*/


.text
.p2align 2

    .global ixheaacd_mps_synt_out_calc_armv7
ixheaacd_mps_synt_out_calc_armv7:

    STMFD           sp!, {r4-r12, r14}
    VPUSH           {D8-D15}
    MOV             R6, #3
    MUL             R7, R0, R6
    ADD             R4, R1, R0, LSL #2
    ADD             R5, R2, R7, LSL #2
    MOV             R6, #5
LOOP1:
    MOV             R8, R0
LOOP2:
    VLD2.32         {D4, D5}, [R3]!
    VLD1.32         {D0, D1}, [R2]!
    VLD1.32         {D2, D3}, [R5]!
    VLD2.32         {D6, D7}, [R3]!


    VMULL.S32       Q4, D0, D4
    VMULL.S32       Q5, D1, D6
    VMULL.S32       Q6, D2, D5
    VMULL.S32       Q7, D3, D7
    VSHRN.S64       D8, Q4, #31
    VSHRN.S64       D9, Q5, #31
    VSHRN.S64       D12, Q6, #31
    VSHRN.S64       D13, Q7, #31


    SUBS            R8, R8, #4
    VST1.32         {D8, D9}, [R1]!
    VST1.32         {D12, D13}, [R4]!
    BGT             LOOP2
    SUBS            R6, R6, #1
    ADD             R1, R1, R0, LSL #2
    ADD             R4, R4, R0, LSL #2
    ADD             R2, R2, R7, LSL #2
    ADD             R5, R5, R7, LSL #2
    BGT             LOOP1
    VPOP            {D8-D15}
    LDMFD           sp!, {r4-r12, r15}




