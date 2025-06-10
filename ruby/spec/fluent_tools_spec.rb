# frozen_string_literal: true

RSpec.describe FluentTools do
  it 'has a version number' do
    expect(FluentTools::VERSION).not_to be nil
  end

  describe '.fluent_to_android' do
    it 'delegates to Converter' do
      converter = instance_double(FluentTools::Converter)
      allow(FluentTools::Converter).to receive(:new).and_return(converter)
      expect(converter).to receive(:fluent_to_android).with('input.ftl', 'output.xml')

      FluentTools.fluent_to_android('input.ftl', 'output.xml')
    end
  end

  describe '.android_to_fluent' do
    it 'delegates to Converter without original_fluent' do
      converter = instance_double(FluentTools::Converter)
      allow(FluentTools::Converter).to receive(:new).and_return(converter)
      expect(converter).to receive(:android_to_fluent).with('input.xml', 'output.ftl', original_fluent: nil)

      FluentTools.android_to_fluent('input.xml', 'output.ftl')
    end

    it 'delegates to Converter with original_fluent' do
      converter = instance_double(FluentTools::Converter)
      allow(FluentTools::Converter).to receive(:new).and_return(converter)
      expect(converter).to receive(:android_to_fluent).with('input.xml', 'output.ftl', original_fluent: 'original.ftl')

      FluentTools.android_to_fluent('input.xml', 'output.ftl', original_fluent: 'original.ftl')
    end
  end
end
