import { debugSafePoint } from "./aone-debug.js";

const shipments = new Map([
  [
    "SHP-1047",
    {
      id: "SHP-1047",
      status: "in_transit",
      destination: "Yokohama",
      lastCheckpoint: "Tokyo distribution center",
    },
  ],
]);

export class ShipmentRepository {
  async findById(shipmentId, workflowId, parentStepId) {
    await debugSafePoint({
      workflowId,
      stepId: "repository-find-by-id",
      parentStepId,
      kind: "method",
      flowStage: "service",
      operation: "ShipmentRepository.findById",
      source: { relativePath: "src/shipment-repository.js", line: 16 },
    });
    await debugSafePoint({
      workflowId,
      stepId: "database-select-shipment",
      parentStepId: "repository-find-by-id",
      kind: "database",
      flowStage: "data",
      operation: "SELECT",
      resource: "shipments",
      source: { relativePath: "src/shipment-repository.js", line: 26 },
      dataPreview: {
        kind: "rowSet",
        typeName: "Shipment",
        fields: [
          { name: "id", valueType: "string", nullable: false },
          { name: "status", valueType: "string", nullable: false },
          { name: "destination", valueType: "string", nullable: false },
        ],
        rowCount: shipments.has(shipmentId) ? 1 : 0,
        nullCount: 0,
        truncated: false,
      },
    });
    return shipments.get(shipmentId) ?? null;
  }
}
