package dev.mercurio.pilot;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.TreeSet;

import org.eclipse.emf.common.util.TreeIterator;
import org.eclipse.emf.ecore.EObject;
import org.eclipse.emf.ecore.resource.Resource;
import org.eclipse.emf.ecore.resource.ResourceSet;
import org.eclipse.xtext.EcoreUtil2;
import org.eclipse.xtext.nodemodel.ICompositeNode;
import org.eclipse.xtext.nodemodel.util.NodeModelUtils;
import org.omg.sysml.interactive.SysMLInteractive;
import org.omg.sysml.lang.sysml.Documentation;
import org.omg.sysml.lang.sysml.Element;
import org.omg.sysml.lang.sysml.Feature;
import org.omg.sysml.lang.sysml.Namespace;
import org.omg.sysml.lang.sysml.Relationship;
import org.omg.sysml.lang.sysml.Specialization;
import org.omg.sysml.lang.sysml.Type;
import org.omg.sysml.util.ElementUtil;

import com.google.gson.GsonBuilder;

public final class PilotStdlibExporter {
    private static final String KERNEL_LIBRARIES = "Kernel Libraries";
    private static final String SYSTEMS_LIBRARY = "Systems Library";
    private static final String DOMAIN_LIBRARIES = "Domain Libraries";

    private PilotStdlibExporter() {
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2) {
            System.err.println("Usage: PilotStdlibExporter <library-root> <output-json>");
            System.exit(2);
        }

        Path libraryRoot = Paths.get(args[0]).toAbsolutePath().normalize();
        Path outputPath = Paths.get(args[1]).toAbsolutePath().normalize();

        System.setProperty("org.eclipse.emf.common.util.ReferenceClearingQueue", "false");

        SysMLInteractive interactive = SysMLInteractive.getInstance();
        interactive.getLibraryIndexCache().setIndexDisabled(true);
        interactive.loadLibrary(libraryRoot.toString());

        ResourceSet resourceSet = interactive.getResourceSet();
        resourceSet.getResources().forEach(resource -> EcoreUtil2.resolveLazyCrossReferences(resource, null));
        ElementUtil.transformAll(resourceSet, false);

        ExportDocument document = exportDocument(libraryRoot, resourceSet);
        if (outputPath.getParent() != null) {
            Files.createDirectories(outputPath.getParent());
        }
        Files.writeString(
            outputPath,
            new GsonBuilder().disableHtmlEscaping().setPrettyPrinting().create().toJson(document),
            StandardCharsets.UTF_8
        );
    }

    private static ExportDocument exportDocument(Path libraryRoot, ResourceSet resourceSet) {
        Map<String, ExportElement> elements = new LinkedHashMap<>();
        Set<RelationshipKey> relationships = new LinkedHashSet<>();
        Set<String> libraryFiles = new TreeSet<>();

        for (Resource resource : resourceSet.getResources()) {
            Path resourcePath = resourcePath(resource);
            if (resourcePath == null || !resourcePath.startsWith(libraryRoot)) {
                continue;
            }

            String relativePath = normalizeRelativePath(libraryRoot.relativize(resourcePath));
            String libraryGroup = libraryGroup(relativePath);
            if (libraryGroup == null) {
                continue;
            }
            libraryFiles.add(relativePath);

            TreeIterator<EObject> iterator = resource.getAllContents();
            while (iterator.hasNext()) {
                EObject object = iterator.next();
                if (!(object instanceof Element element)) {
                    continue;
                }

                String qualifiedName = clean(element.getQualifiedName());
                if (qualifiedName == null) {
                    continue;
                }

                elements.computeIfAbsent(
                    qualifiedName,
                    ignored -> toExportElement(element, qualifiedName, libraryGroup, relativePath)
                );
                collectRelationships(element, qualifiedName, relationships);
            }
        }

        List<ExportElement> exportElements = new ArrayList<>(elements.values());
        exportElements.sort(Comparator.comparing(element -> element.qualified_name));

        List<ExportRelationship> exportRelationships = relationships.stream()
            .filter(relationship -> elements.containsKey(relationship.source))
            .filter(relationship -> elements.containsKey(relationship.target))
            .map(relationship -> new ExportRelationship(relationship.source, relationship.relation, relationship.target))
            .sorted(
                Comparator.comparing((ExportRelationship relationship) -> relationship.source)
                    .thenComparing(relationship -> relationship.relation)
                    .thenComparing(relationship -> relationship.target)
            )
            .toList();

        ExportMetadata metadata = new ExportMetadata();
        metadata.element_count = exportElements.size();
        metadata.relationship_count = exportRelationships.size();
        metadata.library_root = libraryRoot.toString();
        metadata.library_files = new ArrayList<>(libraryFiles);
        metadata.exported_at_utc = Instant.now().toString();
        metadata.pilot_version = pilotVersion();

        ExportDocument document = new ExportDocument();
        document.metadata = metadata;
        document.elements = exportElements;
        document.relationships = exportRelationships;
        return document;
    }

    private static ExportElement toExportElement(
        Element element,
        String qualifiedName,
        String libraryGroup,
        String relativePath
    ) {
        ExportElement export = new ExportElement();
        export.qualified_name = qualifiedName;
        export.kind = element.eClass().getName();
        export.library_group = libraryGroup;
        export.source = new ExportSource(relativePath, startLineOf(element), endLineOf(element));
        export.documentation = documentationOf(element);
        export.properties = propertiesOf(element);
        return export;
    }

    private static List<ExportDocumentationBlock> documentationOf(Element element) {
        List<ExportDocumentationBlock> docs = new ArrayList<>();
        for (Documentation documentation : element.getDocumentation()) {
            String body = clean(documentation.getBody());
            if (body != null) {
                docs.add(new ExportDocumentationBlock("comment", body));
            }
        }
        docs.sort(Comparator.comparing(block -> block.text));
        return docs;
    }

    private static Map<String, Object> propertiesOf(Element element) {
        Map<String, Object> properties = new LinkedHashMap<>();
        putIfPresent(properties, "declared_name", clean(element.getDeclaredName()));
        putIfPresent(properties, "declared_short_name", clean(element.getDeclaredShortName()));
        putIfPresent(properties, "name", clean(element.getName()));
        putIfPresent(properties, "short_name", clean(element.getShortName()));
        properties.put("is_library_element", element.isLibraryElement());

        if (element instanceof Feature feature) {
            properties.put("is_abstract", feature.isAbstract());
            properties.put("is_derived", feature.isDerived());
            properties.put("is_end", feature.isEnd());
            properties.put("is_ordered", feature.isOrdered());
            properties.put("is_unique", feature.isUnique());
            properties.put("is_variable", feature.isVariable());
            if (feature.getDirection() != null) {
                properties.put("direction", feature.getDirection().toString().toLowerCase(Locale.ROOT));
            }
        } else if (element instanceof Type type) {
            properties.put("is_abstract", type.isAbstract());
        } else if (element instanceof Relationship relationship) {
            properties.put("is_implied", relationship.isImplied());
        }

        properties.values().removeIf(Objects::isNull);
        return properties;
    }

    private static void collectRelationships(
        Element element,
        String sourceQualifiedName,
        Set<RelationshipKey> relationships
    ) {
        addRelationship(relationships, sourceQualifiedName, "owner", qualifiedNameOf(element.getOwner()));

        if (element instanceof Namespace namespace) {
            addRelationships(relationships, sourceQualifiedName, "members", namespace.getOwnedMember());
        }

        if (element instanceof Type type) {
            for (Specialization specialization : type.getOwnedSpecialization()) {
                addRelationship(relationships, sourceQualifiedName, "specializes", qualifiedNameOf(specialization.getGeneral()));
            }
            addRelationships(relationships, sourceQualifiedName, "features", type.getOwnedFeature());
        }

        if (element instanceof Feature feature) {
            addRelationships(relationships, sourceQualifiedName, "type", feature.getType());
            addRelationships(relationships, sourceQualifiedName, "featuring_type", feature.getFeaturingType());
            addRelationships(relationships, sourceQualifiedName, "chaining_feature", feature.getChainingFeature());
        }
    }

    private static void addRelationships(
        Set<RelationshipKey> relationships,
        String source,
        String relation,
        Collection<? extends Element> targets
    ) {
        for (Element target : targets) {
            addRelationship(relationships, source, relation, qualifiedNameOf(target));
        }
    }

    private static void addRelationship(
        Set<RelationshipKey> relationships,
        String source,
        String relation,
        String target
    ) {
        if (source == null || target == null || source.equals(target)) {
            return;
        }
        relationships.add(new RelationshipKey(source, relation, target));
    }

    private static String qualifiedNameOf(Element element) {
        return element == null ? null : clean(element.getQualifiedName());
    }

    private static Integer startLineOf(Element element) {
        ICompositeNode node = NodeModelUtils.findActualNodeFor(element);
        return node == null ? null : node.getStartLine();
    }

    private static Integer endLineOf(Element element) {
        ICompositeNode node = NodeModelUtils.findActualNodeFor(element);
        return node == null ? null : node.getEndLine();
    }

    private static Path resourcePath(Resource resource) {
        if (resource.getURI() == null || !resource.getURI().isFile()) {
            return null;
        }
        return Paths.get(resource.getURI().toFileString()).toAbsolutePath().normalize();
    }

    private static String normalizeRelativePath(Path path) {
        return path.toString().replace('\\', '/');
    }

    private static String libraryGroup(String relativePath) {
        if (relativePath.startsWith(KERNEL_LIBRARIES.replace('\\', '/'))) {
            return KERNEL_LIBRARIES;
        }
        if (relativePath.startsWith(SYSTEMS_LIBRARY.replace('\\', '/'))) {
            return SYSTEMS_LIBRARY;
        }
        if (relativePath.startsWith(DOMAIN_LIBRARIES.replace('\\', '/'))) {
            return DOMAIN_LIBRARIES;
        }
        return null;
    }

    private static void putIfPresent(Map<String, Object> properties, String key, String value) {
        if (value != null) {
            properties.put(key, value);
        }
    }

    private static String clean(String value) {
        if (value == null) {
            return null;
        }
        String normalized = value.replace("\r\n", "\n").trim();
        return normalized.isEmpty() ? null : normalized;
    }

    private static String pilotVersion() {
        Package pkg = SysMLInteractive.class.getPackage();
        if (pkg == null) {
            return null;
        }

        String implementationVersion = pkg.getImplementationVersion();
        if (implementationVersion != null && !implementationVersion.isBlank()) {
            return implementationVersion;
        }

        String specificationVersion = pkg.getSpecificationVersion();
        if (specificationVersion != null && !specificationVersion.isBlank()) {
            return specificationVersion;
        }

        return null;
    }

    private static final class RelationshipKey {
        private final String source;
        private final String relation;
        private final String target;

        private RelationshipKey(String source, String relation, String target) {
            this.source = source;
            this.relation = relation;
            this.target = target;
        }

        @Override
        public boolean equals(Object other) {
            if (this == other) {
                return true;
            }
            if (!(other instanceof RelationshipKey key)) {
                return false;
            }
            return source.equals(key.source) && relation.equals(key.relation) && target.equals(key.target);
        }

        @Override
        public int hashCode() {
            return Objects.hash(source, relation, target);
        }
    }

    private static final class ExportDocument {
        private ExportMetadata metadata;
        private List<ExportElement> elements;
        private List<ExportRelationship> relationships;
    }

    private static final class ExportMetadata {
        private int element_count;
        private int relationship_count;
        private String library_root;
        private List<String> library_files;
        private String exported_at_utc;
        private String pilot_version;
    }

    private static final class ExportElement {
        private String qualified_name;
        private String kind;
        private String library_group;
        private ExportSource source;
        private List<ExportDocumentationBlock> documentation;
        private Map<String, Object> properties;
    }

    private static final class ExportSource {
        private final String file;
        private final Integer start_line;
        private final Integer end_line;

        private ExportSource(String file, Integer startLine, Integer endLine) {
            this.file = file;
            this.start_line = startLine;
            this.end_line = endLine;
        }
    }

    private static final class ExportDocumentationBlock {
        private final String kind;
        private final String text;

        private ExportDocumentationBlock(String kind, String text) {
            this.kind = kind;
            this.text = text;
        }
    }

    private static final class ExportRelationship {
        private final String source;
        private final String relation;
        private final String target;

        private ExportRelationship(String source, String relation, String target) {
            this.source = source;
            this.relation = relation;
            this.target = target;
        }
    }
}
