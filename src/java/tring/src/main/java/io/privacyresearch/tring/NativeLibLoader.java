package io.privacyresearch.tring;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

public class NativeLibLoader {

    public static String loadLibrary() throws IOException {
        String libraryName = System.mapLibraryName("ringrtc");
        System.err.println("Will try to load " + libraryName);

        InputStream library = NativeLibLoader.class.getResourceAsStream("/" + libraryName);
        Path target = Files.createTempFile("ringrtc_", "_" + libraryName);
        Files.copy(library, target, StandardCopyOption.REPLACE_EXISTING);
        System.load(target.toString());

        return libraryName;
    }
}
