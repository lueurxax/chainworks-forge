import SwiftUI

#if compiler(>=6.4)
// macOS 27 drag-to-reorder support. SwiftUI's `reorderContainer(for:)` hands back a
// `ReorderDifference` describing the move; this helper applies it in one in-place pass to
// any single-collection array of Identifiable elements. Scoped to single-collection
// containers (CollectionID == ReorderableSingleCollectionIdentifier); sectioned containers
// would route by `destination.collectionID` instead.
extension ReorderDifference where CollectionID == ReorderableSingleCollectionIdentifier {
    func apply<C>(to collection: inout C)
        where C: RangeReplaceableCollection,
              C.Element: Identifiable,
              C.Element.ID == ItemID
    {
        let moving = Set(sources)
        guard !moving.isEmpty else { return }

        var moved: [C.Element] = []
        moved.reserveCapacity(moving.count)
        collection.removeAll { element in
            guard moving.contains(element.id) else { return false }
            moved.append(element)
            return true
        }

        switch destination.position {
        case .before(let id):
            let index = collection.firstIndex { $0.id == id } ?? collection.endIndex
            collection.insert(contentsOf: moved, at: index)
        case .end:
            collection.append(contentsOf: moved)
        }
    }
}
#endif
