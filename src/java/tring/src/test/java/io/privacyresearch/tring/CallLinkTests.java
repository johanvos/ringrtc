package io.privacyresearch.tring;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Assertions;

/**
 *
 * @author johan
 */
public class CallLinkTests {
    static String LINK1 = "mcsm-mqxp-hbpd-sbbq-tzhs-fxcp-qzpx-bzkx";
    static byte[] KEY1 = new byte[]{113, -57, 122, -23, 80, -110, -64, 10, -33, 92, 62, 25, -81, -98, 15, 110};

    @Test
    public void testParseCallLink() {
        TringServiceImpl tsi = new TringServiceImpl();
        tsi.createScope();
        byte[] clb = tsi.getCallLinkBytes(LINK1);
        Assertions.assertArrayEquals(KEY1, clb);
    }

}
