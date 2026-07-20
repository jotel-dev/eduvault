import { NextResponse } from "next/server";
import { getUserFromCookie } from "@/lib/api/auth";
import { withApiHardening } from "@/lib/api/hardening";
import { exportData } from "@/lib/privacy/erasure";
import { auditLog } from "@/lib/api/audit";

export const dynamic = "force-dynamic";

export async function GET(request) {
  return withApiHardening(
    request,
    { route: "privacy_export", rateLimit: { limit: 5, windowMs: 3600_000 } }, // Strict rate limit: 5 per hour
    async () => {
      try {
        const user = await getUserFromCookie(request);
        if (!user) {
          return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
        }

        const data = await exportData(user);

        auditLog({
          event: "privacy_data_exported",
          route: "privacy/export",
          method: "GET",
          userId: user._id,
        });

        // Set headers for download
        return new NextResponse(JSON.stringify(data, null, 2), {
          status: 200,
          headers: {
            "Content-Type": "application/json",
            "Content-Disposition": `attachment; filename="eduvault_export_${user._id}.json"`,
          },
        });
      } catch (error) {
        console.error("Data export failed:", error);
        return NextResponse.json({ error: "Failed to export data" }, { status: 500 });
      }
    }
  );
}
