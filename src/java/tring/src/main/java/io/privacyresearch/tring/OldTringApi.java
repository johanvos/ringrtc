package io.privacyresearch.tring;

import java.util.List;

/**
 *
 * @author johan
 */
@Deprecated
public interface OldTringApi {
    
    void statusCallback(long callId, long peerId, int dir, int type);
    
    void answerCallback(byte[] opaque);

    void iceUpdateCallback(List<byte[]> iceCandidates);
    
}
