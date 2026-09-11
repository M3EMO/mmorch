package com.qtp.bot;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;

/** Diferencial contra el oraculo Python: 300 frases, mismas sesiones, mismo orden. Solo reporta. */
class DifferentialTest {
    @Test
    void contraOraculo() throws Exception {
        Path json = Path.of(System.getProperty("diff.cases"));
        JsonNode cases = new ObjectMapper().readTree(Files.readString(json));
        Bot bot = new Bot(Catalog.load(Path.of("reference", "catalogo.json")));
        int n = 0, okText = 0, okImage = 0, okPaused = 0, okOrder = 0, okAll = 0;
        StringBuilder sample = new StringBuilder();
        for (JsonNode c : cases) {
            n++;
            Reply r = bot.reply(c.get("sid").asText(), c.get("text").asText());
            List<Msg> got = r.replies();
            JsonNode exp = c.get("replies");
            boolean t = got.size() == exp.size();
            boolean im = t;
            for (int i = 0; t && i < exp.size(); i++) {
                String et = exp.get(i).get("text").asText("");
                String gt = got.get(i).text() == null ? "" : got.get(i).text();
                if (!et.equals(gt)) { t = false; if (sample.length() < 1500) sample.append("\n[").append(c.get("text").asText()).append("]\n  esperado: ").append(et.replace("\n", "\\n"), 0, Math.min(140, et.length())).append("\n  obtenido: ").append(gt.replace("\n", "\\n"), 0, Math.min(140, gt.length())); }
                JsonNode ei = exp.get(i).get("image");
                String eiv = (ei == null || ei.isNull()) ? null : ei.asText();
                String giv = got.get(i).image();
                if ((eiv == null) != (giv == null) || (eiv != null && !eiv.equals(giv))) im = false;
            }
            boolean p = r.paused() == c.get("paused").asBoolean();
            JsonNode eo = c.get("order");
            String eov = (eo == null || eo.isNull()) ? null : eo.asText();
            boolean o = (eov == null) == (r.order() == null);
            if (t) okText++;
            if (im) okImage++;
            if (p) okPaused++;
            if (o) okOrder++;
            if (t && im && p && o) okAll++;
        }
        System.out.println("DIFF n=" + n + " texto=" + okText + " imagen=" + okImage + " paused=" + okPaused + " order=" + okOrder + " todo=" + okAll);
        System.out.println("DIFF muestras:" + sample);
    }
}
