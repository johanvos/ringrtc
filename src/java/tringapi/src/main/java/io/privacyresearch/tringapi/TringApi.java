package io.privacyresearch.tringapi;

import java.util.List;

/**
 *
 * @author johan
 */
public interface TringApi {
        
    void statusCallback(long callId, long peerId, int dir, int type);
    
    void answerCallback(byte[] opaque);

    void offerCallback(byte[] opaque);

    void iceUpdateCallback(List<byte[]> iceCandidates);

    void groupCallUpdateRing(byte[] groupId, long ringId, byte[] senderBytes, byte status);

}
