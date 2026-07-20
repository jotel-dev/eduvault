import { NextResponse } from "next/server";
import { getUserFromCookie } from "@/lib/api/auth";
import { withApiHardening } from "@/lib/api/hardening";
import { initiateDeletion } from "@/lib/privacy/erasure";
import { auditLog } from "@/lib/api/audit";
import { cookies } from "next/headers";

export const dynamic = "force-dynamic";

export async function POST(request) {
  return withApiHardening(
    request,
    { route: "privacy_delete", rateLimit: { limit: 3, windowMs: 86400_000 } }, // Strict rate limit: 3 per day
    async () => {
      try {
        const user = await getUserFromCookie(request);
        if (!user) {
          return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
        }

        const body = await request.json();
        if (body.confirmation !== "I understand that this action is permanent and cannot be undone.") {
          return NextResponse.json({ error: "Invalid confirmation phrase" }, { status: 400 });
        }

        try {
          await initiateDeletion(user);
        } catch (delError) {
          if (delError.message.includes("legal hold")) {
            return NextResponse.json({ error: delError.message }, { status: 403 });
          }
          throw delError;
        }

        auditLog({
          event: "privacy_account_deleted",
          route: "privacy/delete",
          method: "POST",
          userId: user._id,
        });

        // Clear session cookie since account is deleted
        const cookieStore = await cookies();
        cookieStore.delete("session");

        return NextResponse.json({ success: true, message: "Account data erased successfully" });
      } catch (error) {
        console.error("Account deletion failed:", error);
        return NextResponse.json({ error: "Failed to erase account" }, { status: 500 });
      }
    }
  );
}
