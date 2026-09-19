import { ShipmentRepository } from "./shipment-repository.js";
import { debugSafePoint } from "./aone-debug.js";

const shipmentRepository = new ShipmentRepository();

export async function getShipment(shipmentId, workflowId, parentStepId) {
  await debugSafePoint({
    workflowId,
    stepId: "service-get-shipment",
    parentStepId,
    kind: "method",
    flowStage: "service",
    operation: "getShipment",
    source: { relativePath: "src/shipment-service.js", line: 6 },
  });
  const shipment = await shipmentRepository.findById(
    shipmentId,
    workflowId,
    "service-get-shipment",
  );
  await debugSafePoint({
    workflowId,
    stepId: "branch-shipment-found",
    parentStepId: "database-select-shipment",
    kind: "branch",
    flowStage: "backend",
    branchOutcome: shipment ? "then" : "else",
    source: { relativePath: "src/shipment-service.js", line: 21 },
  });
  if (!shipment) {
    return { status: 404, body: { error: "Shipment not found" } };
  }

  return {
    status: 200,
    body: {
      ...shipment,
      nextAction: shipment.status === "in_transit" ? "Monitor arrival scan" : "Review exception",
    },
  };
}
