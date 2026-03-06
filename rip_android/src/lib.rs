//! Android JNI bindings for Rip receipt markup rendering.
//!
//! Provides JNI functions that wrap the `rip` unified API.
//! The Kotlin wrapper (`Rip.kt`) handles resource fetching and
//! image decoding — users just call suspend functions on the `Rip` object.

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jbyteArray, jint, jobject, jstring};
use rip_parser::{parse, collect_resources, RenderResources, ImageData};

// ─── Resource discovery ─────────────────────────────────────────────

/// Parse markup and return the external resource URLs it references.
///
/// Returns a `ResourceUrls` object with `fonts: Array<String>` and `images: Array<String>`.
#[no_mangle]
pub extern "system" fn Java_com_zachzurn_rip_RipNative_getResources<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    source: JString<'local>,
) -> jobject {
    let source: String = env.get_string(&source).unwrap().into();
    let nodes = parse(&source);
    let urls = collect_resources(&nodes);

    // Build ResourceUrls(fonts: Array<String>, images: Array<String>)
    let string_class = env.find_class("java/lang/String").unwrap();

    let fonts_arr = env
        .new_object_array(urls.fonts.len() as i32, &string_class, &JObject::null())
        .unwrap();
    for (i, f) in urls.fonts.iter().enumerate() {
        let s = env.new_string(f).unwrap();
        env.set_object_array_element(&fonts_arr, i as i32, s)
            .unwrap();
    }

    let images_arr = env
        .new_object_array(urls.images.len() as i32, &string_class, &JObject::null())
        .unwrap();
    for (i, img) in urls.images.iter().enumerate() {
        let s = env.new_string(img).unwrap();
        env.set_object_array_element(&images_arr, i as i32, s)
            .unwrap();
    }

    let cls = env.find_class("com/zachzurn/rip/ResourceUrls").unwrap();
    let obj = env
        .new_object(
            cls,
            "([Ljava/lang/String;[Ljava/lang/String;)V",
            &[
                (&fonts_arr).into(),
                (&images_arr).into(),
            ],
        )
        .unwrap();

    obj.into_raw()
}

// ─── Renderers ──────────────────────────────────────────────────────

/// Render markup to a standalone HTML document.
#[no_mangle]
pub extern "system" fn Java_com_zachzurn_rip_RipNative_renderHtml<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    source: JString<'local>,
) -> jstring {
    let source: String = env.get_string(&source).unwrap().into();
    let nodes = parse(&source);
    let html = rip::render_html(&nodes);
    env.new_string(html).unwrap().into_raw()
}

/// Render markup to plain text (monospace ASCII).
#[no_mangle]
pub extern "system" fn Java_com_zachzurn_rip_RipNative_renderText<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    source: JString<'local>,
) -> jstring {
    let source: String = env.get_string(&source).unwrap().into();
    let nodes = parse(&source);
    let text = rip::render_text(&nodes);
    env.new_string(text).unwrap().into_raw()
}

/// Render markup to 8-bit grayscale pixels (anti-aliased).
///
/// Returns a `PixelOutput(width, height, pixels, dirtyRows)`.
#[no_mangle]
pub extern "system" fn Java_com_zachzurn_rip_RipNative_renderPixels<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    source: JString<'local>,
    resources_obj: JObject<'local>,
) -> jobject {
    let source: String = env.get_string(&source).unwrap().into();
    let nodes = parse(&source);
    let resources = jni_to_resources(&mut env, &resources_obj);

    match rip::render_luma8(&nodes, &resources) {
        Ok(output) => pixel_output_to_jni(&mut env, &output),
        Err(e) => {
            env.throw_new("com/zachzurn/rip/RipRenderException", e.to_string())
                .unwrap();
            std::ptr::null_mut()
        }
    }
}

/// Render markup to 1-bit packed pixels (thresholded black/white).
///
/// Pixels are MSB-first packed, `ceil(width/8)` bytes per row.
#[no_mangle]
pub extern "system" fn Java_com_zachzurn_rip_RipNative_renderRaster<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    source: JString<'local>,
    resources_obj: JObject<'local>,
) -> jobject {
    let source: String = env.get_string(&source).unwrap().into();
    let nodes = parse(&source);
    let resources = jni_to_resources(&mut env, &resources_obj);

    match rip::render_luma1(&nodes, &resources) {
        Ok(output) => pixel_output_to_jni(&mut env, &output),
        Err(e) => {
            env.throw_new("com/zachzurn/rip/RipRenderException", e.to_string())
                .unwrap();
            std::ptr::null_mut()
        }
    }
}

/// Render markup to ESC/POS binary commands for thermal printers.
#[no_mangle]
pub extern "system" fn Java_com_zachzurn_rip_RipNative_renderEscpos<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    source: JString<'local>,
    resources_obj: JObject<'local>,
) -> jbyteArray {
    let source: String = env.get_string(&source).unwrap().into();
    let nodes = parse(&source);
    let resources = jni_to_resources(&mut env, &resources_obj);
    let bytes = rip::render_escpos(&nodes, &resources);

    let arr = env.new_byte_array(bytes.len() as i32).unwrap();
    // Safety: reinterpret &[u8] as &[i8] for JNI — same memory layout
    let signed: &[i8] = unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const i8, bytes.len()) };
    env.set_byte_array_region(&arr, 0, signed).unwrap();
    arr.into_raw()
}

// ─── JNI ↔ Rust conversion helpers ──────────────────────────────────

/// Convert a `rip::PixelOutput` to a Java `PixelOutput` object.
fn pixel_output_to_jni(env: &mut JNIEnv, output: &rip::PixelOutput) -> jobject {
    // Create pixels byte array
    let pixels_arr = env.new_byte_array(output.pixels.len() as i32).unwrap();
    let signed: &[i8] = unsafe {
        std::slice::from_raw_parts(output.pixels.as_ptr() as *const i8, output.pixels.len())
    };
    env.set_byte_array_region(&pixels_arr, 0, signed).unwrap();

    // Create dirtyRows boolean array
    let dirty_arr = env.new_boolean_array(output.dirty_rows.len() as i32).unwrap();
    let bools: Vec<u8> = output.dirty_rows.iter().map(|&b| b as u8).collect();
    env.set_boolean_array_region(&dirty_arr, 0, &bools).unwrap();

    // Construct PixelOutput(width: Int, height: Int, pixels: ByteArray, dirtyRows: BooleanArray)
    let cls = env.find_class("com/zachzurn/rip/PixelOutput").unwrap();
    let obj = env
        .new_object(
            cls,
            "(II[B[Z)V",
            &[
                (output.width as jint).into(),
                (output.height as jint).into(),
                (&pixels_arr).into(),
                (&dirty_arr).into(),
            ],
        )
        .unwrap();

    obj.into_raw()
}

/// Convert a Java resources object to `RenderResources`.
///
/// Expected Kotlin shape:
/// ```kotlin
/// class RipResourcesJni(
///     val images: HashMap<String, RipImageData>,  // RipImageData(width, height, pixels)
///     val fonts: HashMap<String, ByteArray>
/// )
/// ```
fn jni_to_resources(env: &mut JNIEnv, obj: &JObject) -> RenderResources {
    let mut resources = RenderResources::default();

    if obj.is_null() {
        return resources;
    }

    // Read images: HashMap<String, RipImageData>
    let images_obj = env
        .get_field(obj, "images", "Ljava/util/HashMap;")
        .unwrap()
        .l()
        .unwrap();

    if !images_obj.is_null() {
        // Get entrySet -> iterator
        let entry_set = env
            .call_method(&images_obj, "entrySet", "()Ljava/util/Set;", &[])
            .unwrap()
            .l()
            .unwrap();
        let iterator = env
            .call_method(&entry_set, "iterator", "()Ljava/util/Iterator;", &[])
            .unwrap()
            .l()
            .unwrap();

        while env
            .call_method(&iterator, "hasNext", "()Z", &[])
            .unwrap()
            .z()
            .unwrap()
        {
            let entry = env
                .call_method(&iterator, "next", "()Ljava/lang/Object;", &[])
                .unwrap()
                .l()
                .unwrap();

            let key_obj = env
                .call_method(&entry, "getKey", "()Ljava/lang/Object;", &[])
                .unwrap()
                .l()
                .unwrap();
            let key: String = env.get_string((&key_obj).into()).unwrap().into();

            let val_obj = env
                .call_method(&entry, "getValue", "()Ljava/lang/Object;", &[])
                .unwrap()
                .l()
                .unwrap();

            // Read RipImageData fields
            let width = env
                .get_field(&val_obj, "width", "I")
                .unwrap()
                .i()
                .unwrap() as u32;
            let height = env
                .get_field(&val_obj, "height", "I")
                .unwrap()
                .i()
                .unwrap() as u32;
            let pixels_obj: JByteArray = env
                .get_field(&val_obj, "pixels", "[B")
                .unwrap()
                .l()
                .unwrap()
                .into();
            let pixels = env.convert_byte_array(pixels_obj).unwrap();

            resources.images.insert(key, ImageData { width, height, pixels });
        }
    }

    // Read fonts: HashMap<String, ByteArray>
    let fonts_obj = env
        .get_field(obj, "fonts", "Ljava/util/HashMap;")
        .unwrap()
        .l()
        .unwrap();

    if !fonts_obj.is_null() {
        let entry_set = env
            .call_method(&fonts_obj, "entrySet", "()Ljava/util/Set;", &[])
            .unwrap()
            .l()
            .unwrap();
        let iterator = env
            .call_method(&entry_set, "iterator", "()Ljava/util/Iterator;", &[])
            .unwrap()
            .l()
            .unwrap();

        while env
            .call_method(&iterator, "hasNext", "()Z", &[])
            .unwrap()
            .z()
            .unwrap()
        {
            let entry = env
                .call_method(&iterator, "next", "()Ljava/lang/Object;", &[])
                .unwrap()
                .l()
                .unwrap();

            let key_obj = env
                .call_method(&entry, "getKey", "()Ljava/lang/Object;", &[])
                .unwrap()
                .l()
                .unwrap();
            let key: String = env.get_string((&key_obj).into()).unwrap().into();

            let val_obj: JByteArray = env
                .call_method(&entry, "getValue", "()Ljava/lang/Object;", &[])
                .unwrap()
                .l()
                .unwrap()
                .into();
            let bytes = env.convert_byte_array(val_obj).unwrap();

            resources.fonts.insert(key, bytes);
        }
    }

    resources
}
