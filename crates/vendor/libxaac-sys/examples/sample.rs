use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::ffi::{CStr, c_void};
use std::mem::{align_of, size_of, zeroed};
use std::ptr;

use libxaac_sys::{
    AOT_AAC_LC, DEFAULT_MEM_ALIGN_8, IA_API_CMD_GET_API_SIZE, IA_MEMTYPE_INPUT, IA_MEMTYPE_OUTPUT,
    ia_lib_info_struct, ixheaacd_dec_api, ixheaacd_get_lib_id_strings, ixheaace_delete,
    ixheaace_get_lib_id_strings, ixheaace_process, ixheaace_user_config_struct, ixheaace_version,
};

fn main() {
    print_library_versions();
    print_decoder_api_size();

    let (mut config, pcm_input_size) = build_encoder_config();

    let create_status = unsafe {
        libxaac_sys::ixheaace_create(
            (&mut config.input_config as *mut _) as *mut c_void,
            (&mut config.output_config as *mut _) as *mut c_void,
        )
    };
    assert_eq!(create_status, 0, "ixheaace_create failed: {create_status}");

    let process_obj = config.output_config.pv_ia_process_api_obj;
    let in_buf = config.output_config.mem_info_table[IA_MEMTYPE_INPUT as usize].mem_ptr as *mut u8;
    let out_buf =
        config.output_config.mem_info_table[IA_MEMTYPE_OUTPUT as usize].mem_ptr as *const u8;

    unsafe {
        ptr::write_bytes(in_buf, 0, pcm_input_size);
    }

    let asc_len = config.output_config.i_out_bytes as usize;
    println!("encoder input frame bytes: {pcm_input_size}");
    if asc_len != 0 {
        println!("audio specific config bytes: {:02x?}", unsafe {
            std::slice::from_raw_parts(out_buf, asc_len)
        });
    }

    let process_status = unsafe {
        ixheaace_process(
            process_obj,
            (&mut config.input_config as *mut _) as *mut c_void,
            (&mut config.output_config as *mut _) as *mut c_void,
        )
    };
    assert_eq!(
        process_status, 0,
        "ixheaace_process failed: {process_status}"
    );

    let encoded_len = config.output_config.i_out_bytes as usize;
    let encoded = unsafe { std::slice::from_raw_parts(out_buf, encoded_len) };
    println!("encoded silent AAC frame bytes: {encoded_len}");
    println!(
        "first frame prefix: {:02x?}",
        &encoded[..encoded.len().min(16)]
    );

    let delete_status =
        unsafe { ixheaace_delete((&mut config.output_config as *mut _) as *mut c_void) };
    assert_eq!(delete_status, 0, "ixheaace_delete failed: {delete_status}");
}

fn build_encoder_config() -> (ixheaace_user_config_struct, usize) {
    let mut config: ixheaace_user_config_struct = unsafe { zeroed() };

    config.output_config.malloc_xheaace = Some(xaac_alloc);
    config.output_config.free_xheaace = Some(xaac_free);

    config.input_config.ui_pcm_wd_sz = 16;
    config.input_config.i_bitrate = 48_000;
    config.input_config.frame_length = 1024;
    config.input_config.aot = AOT_AAC_LC as i32;
    config.input_config.i_channels = 2;
    config.input_config.i_samp_freq = 44_100;
    config.input_config.i_native_samp_freq = 44_100;
    config.input_config.i_mps_tree_config = -1;
    config.input_config.i_use_mps = 0;
    config.input_config.i_use_adts = 0;
    config.input_config.i_use_es = 1;
    config.input_config.random_access_interval = 0;

    config.input_config.aac_config.sample_rate = 44_100;
    config.input_config.aac_config.bitrate = 48_000;
    config.input_config.aac_config.num_channels_in = 2;
    config.input_config.aac_config.num_channels_out = 2;
    config.input_config.aac_config.use_tns = 1;
    config.input_config.aac_config.bitreservoir_size = 768;
    config.input_config.aac_config.length = 1024 * 2 * 2;

    let pcm_input_size = config.input_config.frame_length as usize
        * config.input_config.i_channels as usize
        * (config.input_config.ui_pcm_wd_sz as usize / 8);

    (config, pcm_input_size)
}

fn print_library_versions() {
    let mut enc_version: ixheaace_version = unsafe { zeroed() };
    let mut dec_version = unsafe { zeroed::<ia_lib_info_struct>() };

    let enc_status =
        unsafe { ixheaace_get_lib_id_strings((&mut enc_version as *mut _) as *mut c_void) };
    assert_eq!(
        enc_status, 0,
        "ixheaace_get_lib_id_strings failed: {enc_status}"
    );
    unsafe { ixheaacd_get_lib_id_strings((&mut dec_version as *mut _) as *mut c_void) };

    println!(
        "encoder: {} {}",
        c_string(enc_version.p_lib_name),
        c_string(enc_version.p_version_num)
    );
    println!(
        "decoder: {} {}",
        c_string(dec_version.p_lib_name),
        c_string(dec_version.p_version_num)
    );
}

fn print_decoder_api_size() {
    let mut api_size = 0u32;
    let status = unsafe {
        ixheaacd_dec_api(
            ptr::null_mut(),
            IA_API_CMD_GET_API_SIZE as i32,
            0,
            (&mut api_size as *mut _) as *mut c_void,
        )
    };
    assert_eq!(status, 0, "ixheaacd_dec_api(GET_API_SIZE) failed: {status}");
    println!("decoder api object size: {api_size} bytes");
}

fn c_string(ptr: *mut i8) -> String {
    if ptr.is_null() {
        return "<null>".to_owned();
    }

    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

unsafe extern "C" fn xaac_alloc(size: u32, alignment: u32) -> *mut c_void {
    let requested_align = alignment.max(DEFAULT_MEM_ALIGN_8).next_power_of_two() as usize;
    let header_words = 2 * size_of::<usize>();
    let total_size = size as usize + requested_align + header_words;
    let layout = Layout::from_size_align(total_size, align_of::<usize>()).unwrap();
    let base = unsafe { alloc_zeroed(layout) };
    if base.is_null() {
        return ptr::null_mut();
    }

    let aligned_addr =
        (base as usize + header_words + requested_align - 1) & !(requested_align - 1);
    let aligned = aligned_addr as *mut u8;
    let meta = unsafe { aligned.cast::<usize>().sub(2) };
    unsafe {
        meta.write(base as usize);
        meta.add(1).write(total_size);
    }
    aligned.cast()
}

unsafe extern "C" fn xaac_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }

    let meta = unsafe { (ptr as *mut usize).sub(2) };
    let base = unsafe { meta.read() } as *mut u8;
    let total_size = unsafe { meta.add(1).read() };
    let layout = Layout::from_size_align(total_size, align_of::<usize>()).unwrap();
    unsafe {
        dealloc(base, layout);
    }
}
