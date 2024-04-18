package io.privacyresearch.tringapi;

public class TringFrame {
    
    public final int width, height, pixelFormat;
    public final byte[] data;
    
    public TringFrame(int width, int height, int pixelFormat, byte[] data) {
        this.width = width;
        this.height = height;
        this.pixelFormat = pixelFormat;
        this.data = data;
    }
}
