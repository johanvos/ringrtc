package io.privacyresearch.tring;

//import io.privacyresearch.tringapi.TringApi;
//import io.privacyresearch.tringapi.TringService;
//
//import java.util.List;
//import java.util.Optional;
//import java.util.ServiceLoader;

/**
 *
 * @author johan
 */
@Deprecated
public class OldTringBridge {
    // ready to be removed in next commit
//    
//    private TringService service;
//    
//    public OldTringBridge(TringApi api) {
//        ServiceLoader<TringService> loader = ServiceLoader.load(TringService.class);
//        Optional<TringService> serviceOpt = loader.findFirst();
//        this.service = serviceOpt.get();
//        this.service.setApi(api);
//    }
//
//    public void acceptCall() {
//        service.acceptCall();
//    }
//
//    public void proceed(long callId) {
//        service.proceed(callId);
//    }
//
//    public void receivedIce(long callId, int senderDeviceId, List<byte[]> ice) {
//        receivedIce(callId, senderDeviceId, ice);
//    }
//
//    public void receivedOffer(String peerId, long callId, int senderDeviceId, int receiverDeviceId,
//            byte[] senderKey, byte[] receiverKey, byte[] opaque) {
//        receivedOffer(peerId, callId, senderDeviceId, receiverDeviceId, senderKey, receiverKey, opaque);
//    }

}
