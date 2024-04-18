package io.privacyresearch.tring;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

public class NativeLibLoader {

    public static String loadLibrary() throws IOException {
        String libName = "/"+System.mapLibraryName("ringrtc");
        System.err.println("Will try to load "+libName);
        InputStream is = NativeLibLoader.class.getResourceAsStream(libName);
        Path target = Files.createTempFile("", "");
        Files.copy(is, target, StandardCopyOption.REPLACE_EXISTING);
        System.load(target.toString());
        return libName;
    }
}
